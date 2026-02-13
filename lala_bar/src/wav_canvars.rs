mod pipewire;

use iced::mouse;
use iced::widget::canvas;
use iced::widget::canvas::{Geometry, Path};
use iced::{Point, Rectangle, Renderer, Theme};

pub use pipewire::{Matrix, MatrixFixed, PwEvent, listen_pw};

use crate::wav_canvars::pipewire::{FFT_SIZE, MIN_FREQ, POINTS_PER_OCTAVE};

#[derive(Debug)]
pub struct LineData {
    data: Vec<Point>,
    color: iced::Color,
}

#[derive(Debug)]
pub struct LineDatas {
    matrix: MatrixFixed,
    spectrum: Vec<f32>,
    rate: u32,
}

const COLOR_ALL: &[iced::Color] = &[
    iced::Color::WHITE,
    iced::Color::from_rgb(0.7, 0.4, 1.),
    iced::Color::from_rgb(0., 0.5, 1.),
    iced::Color::from_rgb(0.5, 0.5, 0.5),
];

impl LineDatas {
    pub fn new() -> Self {
        Self {
            matrix: MatrixFixed::new(500, 2),
            spectrum: vec![0.; FFT_SIZE],
            rate: 50000,
        }
    }

    pub fn append_data(&mut self, matrix: Matrix) {
        self.matrix.append(matrix);
    }

    pub fn reset_format(&mut self, len: usize, channel: usize, rate: u32) {
        self.matrix = MatrixFixed::new(len, channel);
        self.rate = rate;
    }

    fn generate_spectrum(&self, size: iced::Size) -> LineData {
        let rate = self.rate as f64;

        let log_min = MIN_FREQ.log10();
        let log_max = rate.log10();

        let octaves = (log_max - log_min) / (2.0_f64).log10();
        let num_points = (octaves * POINTS_PER_OCTAVE as f64).round().max(32.0) as usize;
        let step = size.width as f64 / num_points as f64;
        let color = COLOR_ALL[1];
        let data: Vec<Point> = (0..num_points)
            .zip(&self.spectrum)
            .map(|(index, db)| Point::new(index as f32 * step as f32, db * -3.))
            .collect();

        LineData { data, color }
    }
    pub fn set_spectrum(&mut self, spectrum: Vec<f32>) {
        self.spectrum = spectrum;
    }

    pub fn generate_datas(&self, size: iced::Size) -> Vec<LineData> {
        let len = self.matrix.len();
        let width = size.width;
        let height = size.height;
        let step = width / len as f32;
        let datas = self.matrix.data();
        let mut output: Vec<LineData> = vec![];
        for (index, data) in datas.iter().enumerate() {
            let color = COLOR_ALL[index % COLOR_ALL.len()];
            let data: Vec<Point> = data
                .iter()
                .enumerate()
                .map(|(index, wav)| Point::new(index as f32 * step, *wav * height))
                .collect();
            output.push(LineData { data, color });
        }
        output
    }
}

#[derive(Debug)]
pub struct WavState {
    line_cache: canvas::Cache,
    datas: LineDatas,
}

impl WavState {
    pub fn new() -> WavState {
        WavState {
            line_cache: canvas::Cache::default(),
            datas: LineDatas::new(),
        }
    }

    pub fn set_spectrum(&mut self, spectrum: Vec<f32>) {
        self.datas.set_spectrum(spectrum);
    }

    #[allow(unused)]
    pub fn generate_datas(&self, size: iced::Size) -> Vec<LineData> {
        self.datas.generate_datas(size)
    }

    pub fn generate_spectrum(&self, size: iced::Size) -> LineData {
        self.datas.generate_spectrum(size)
    }

    pub fn update_canvas(&mut self) {
        self.line_cache.clear();
    }

    pub fn append_data(&mut self, matrix: Matrix) {
        self.datas.append_data(matrix);
    }
    pub fn reset_format(&mut self, len: usize, channel: usize, rate: u32) {
        self.datas.reset_format(len, channel, rate);
    }
}

impl<Message> canvas::Program<Message> for WavState {
    type State = Vec<LineData>;

    fn update(
        &self,
        state: &mut Self::State,
        _event: &iced::Event,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        *state = vec![self.generate_spectrum(bounds.size())];
        None
    }
    fn draw(
        &self,
        datas: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let background = self.line_cache.draw(renderer, bounds.size(), |frame| {
            for data in datas {
                let dot = Path::new(|path| {
                    for p in &data.data {
                        path.line_to(*p);
                    }
                    path.line_to(Point {
                        x: frame.width(),
                        y: 0.,
                    });
                    path.line_to(Point { x: 0., y: 0. });
                    path.close();
                });

                let translation = Point {
                    x: Point::ORIGIN.x,
                    y: frame.height(),
                };
                frame.translate(translation - Point::ORIGIN);

                frame.fill(&dot, data.color);

                frame.translate(Point::ORIGIN - translation);
            }
        });

        vec![background]
    }
}
