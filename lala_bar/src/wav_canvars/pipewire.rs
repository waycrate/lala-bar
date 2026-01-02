use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender;
use pipewire as pw;
use pw::{properties::properties, spa};
use spa::param::format::{MediaSubtype, MediaType};
use spa::param::format_utils;
use spa::pod::Pod;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::mem;
use std::slice::Chunks;
use std::sync::mpsc::{Sender as StdSender, channel};

#[derive(Debug, Clone)]
pub struct PwAudioInfo {
    channels: u32,
    rate: u32,
}

impl PwAudioInfo {
    pub fn channels(&self) -> u32 {
        self.channels
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
    fn new(channels: u32, rate: u32) -> Self {
        Self { channels, rate }
    }
}

#[derive(Debug, Clone)]
pub enum PwEvent {
    FormatChange(PwAudioInfo),
    DataNew(Matrix<f32>),
    PwErr,
}

struct UserData {
    format: spa::param::audio::AudioInfoRaw,
    sender: StdSender<PwEvent>,
}

pub fn listen_pw() -> iced::Subscription<PwEvent> {
    iced::Subscription::run(|| {
        iced::stream::channel(100, |mut output: Sender<PwEvent>| async move {
            let (sync_sender, sync_receiver) = channel();
            std::thread::spawn(move || {
                connect(sync_sender);
            });
            loop {
                let Ok(data) = sync_receiver.recv() else {
                    let _ = output.send(PwEvent::PwErr).await;
                    break;
                };
                let _ = output.send(data).await;
            }
        })
    })
}

#[derive(Debug, Clone)]
pub struct MatrixFixed<T = f32>
where
    T: Clone + Copy + Default,
{
    inner: Vec<VecDeque<T>>,
    len: usize,
    channel: usize,
}

impl<T> MatrixFixed<T>
where
    T: Clone + Copy + Default,
{
    pub fn new(len: usize, channel: usize) -> Self {
        Self {
            inner: vec![vec![Default::default(); len].into(); channel],
            len,
            channel,
        }
    }
    pub fn channel(&self) -> usize {
        self.channel
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn data(&self) -> &[VecDeque<T>] {
        self.inner.as_slice()
    }
    pub fn append(&mut self, matrix: Matrix<T>) {
        assert_eq!(matrix.channel(), self.channel());
        let chunks = matrix.chunks(1);
        for chunk in chunks {
            for (data, channel_data) in chunk.iter().zip(&mut self.inner) {
                channel_data.push_back(data[0]);
                channel_data.pop_front();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Matrix<T = f32>
where
    T: Clone + Copy,
{
    inner: Vec<Vec<T>>,
}

struct MatrixChunks<'a, T>
where
    T: Clone + Copy,
{
    inner: Vec<Chunks<'a, T>>,
}

impl<'a, T> Iterator for MatrixChunks<'a, T>
where
    T: Clone + Copy,
{
    type Item = Vec<&'a [T]>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut outputs = vec![];
        for chunk in &mut self.inner {
            let data = chunk.next()?;
            outputs.push(data);
        }
        Some(outputs)
    }
}

impl<T> Matrix<T>
where
    T: Clone + Copy,
{
    fn init(inner: Vec<Vec<T>>) -> Self {
        Self { inner }
    }
    fn channel(&self) -> usize {
        self.inner.len()
    }
    fn chunks<'a>(&'a self, chunk_size: usize) -> MatrixChunks<'a, T> {
        let mut chunks = vec![];
        for data in &self.inner {
            chunks.push(data.chunks(chunk_size));
        }
        MatrixChunks { inner: chunks }
    }
}

fn connect(sender: StdSender<PwEvent>) {
    if connect_inner(sender.clone()).is_err() {
        let _ = sender.send(PwEvent::PwErr);
    }
}

fn connect_inner(sender: StdSender<PwEvent>) -> Result<(), pw::Error> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;

    let data = UserData {
        format: Default::default(),
        sender,
    };

    /* Create a simple stream, the simple stream manages the core and remote
     * objects for you if you don't need to deal with them.
     *
     * If you plan to autoconnect your stream, you need to provide at least
     * media, category and role properties.
     *
     * Pass your events and a user_data pointer as the last arguments. This
     * will inform you about the stream state. The most important event
     * you need to listen to is the process event where you need to produce
     * the data.
     */
    let props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::STREAM_CAPTURE_SINK => "true",
    };

    // uncomment if you want to capture from the sink monitor ports
    // props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");

    let stream = pw::stream::StreamBox::new(&core, "audio-capture", props)?;

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .param_changed(|_, user_data, id, param| {
            // NULL means to clear the format
            let Some(param) = param else {
                return;
            };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }

            let (media_type, media_subtype) = match format_utils::parse_format(param) {
                Ok(v) => v,
                Err(_) => return,
            };

            // only accept raw audio
            if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                return;
            }

            // call a helper function to parse the format for us.
            user_data
                .format
                .parse(param)
                .expect("Failed to parse param changed to AudioInfoRaw");

            let pw_audio_info =
                PwAudioInfo::new(user_data.format.channels(), user_data.format.rate());
            let _ = user_data.sender.send(PwEvent::FormatChange(pw_audio_info));
            tracing::info!(
                "capturing rate:{} channels:{}",
                user_data.format.rate(),
                user_data.format.channels()
            );
        })
        .process(|stream, user_data| match stream.dequeue_buffer() {
            None => tracing::warn!("out of buffers"),
            Some(mut buffer) => {
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }

                let data = &mut datas[0];
                let n_channels = user_data.format.channels();
                let n_samples = data.chunk().size() / (mem::size_of::<f32>() as u32);

                let Some(samples) = data.data() else {
                    return;
                };
                let mut matrix_inner =
                    vec![vec![0.; (n_samples / n_channels) as usize]; n_channels as usize];
                for c in 0..n_channels {
                    for (index, n) in (c..n_samples).step_by(n_channels as usize).enumerate() {
                        let start = n as usize * mem::size_of::<f32>();
                        let end = start + mem::size_of::<f32>();
                        let chan = &samples[start..end];
                        let f = f32::from_le_bytes(chan.try_into().unwrap());
                        matrix_inner[c as usize][index] = f;
                    }
                }
                let matrix = Matrix {
                    inner: matrix_inner,
                };
                for data in matrix.chunks(80) {
                    let data_new: Vec<Vec<f32>> = data.iter().map(|data| data.to_vec()).collect();
                    let data_chunk: Matrix<f32> = Matrix::init(data_new);
                    let _ = user_data.sender.send(PwEvent::DataNew(data_chunk));
                }
            }
        })
        .register()?;

    /* Make one parameter with the supported formats. The SPA_PARAM_EnumFormat
     * id means that this is a format enumeration (of 1 value).
     * We leave the channels and rate empty to accept the native graph
     * rate and channels. */
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    let obj = pw::spa::pod::Object {
        type_: pw::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: pw::spa::param::ParamType::EnumFormat.as_raw(),
        properties: audio_info.into(),
    };
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner();

    let mut params = [Pod::from_bytes(&values).unwrap()];

    /* Now connect this stream. We ask that our process function is
     * called in a realtime thread. */
    stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT
            | pw::stream::StreamFlags::MAP_BUFFERS
            | pw::stream::StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    // and wait while we let things run
    mainloop.run();
    Ok(())
}
