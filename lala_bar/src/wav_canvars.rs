mod pipewire;

use iced::mouse;
use iced::widget::canvas;
use iced::widget::canvas::{Geometry, Path, Stroke, stroke};
use iced::{Point, Rectangle, Renderer, Theme};

pub use pipewire::{Matrix, MatrixFixed, PwEvent, listen_pw};
#[derive(Debug)]
pub struct LineData {
    data: Vec<Point>,
    color: iced::Color,
}

#[derive(Debug)]
pub struct LineDatas {
    matrix: MatrixFixed,
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
        }
    }

    pub fn append_data(&mut self, matrix: Matrix) {
        self.matrix.append(matrix);
    }

    pub fn reset_matrix(&mut self, len: usize, channel: usize) {
        self.matrix = MatrixFixed::new(len, channel);
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

    pub fn generate_datas(&self, size: iced::Size) -> Vec<LineData> {
        self.datas.generate_datas(size)
    }

    pub fn update_canvas(&mut self) {
        self.line_cache.clear();
    }

    pub fn append_data(&mut self, matrix: Matrix) {
        self.datas.append_data(matrix);
    }
    pub fn reset_matrix(&mut self, len: usize, channel: usize) {
        self.datas.reset_matrix(len, channel);
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
        *state = self.generate_datas(bounds.size());
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
            let center = frame.center();
            for data in datas {
                let stars = Path::new(|path| {
                    for p in &data.data {
                        path.line_to(*p);
                    }
                });

                let translation = Point {
                    x: Point::ORIGIN.x,
                    y: center.y,
                };
                frame.translate(translation - Point::ORIGIN);
                frame.stroke(
                    &stars,
                    Stroke {
                        width: 3.,
                        style: stroke::Style::Solid(data.color),
                        ..Default::default()
                    },
                );
                frame.translate(Point::ORIGIN - translation);
            }
        });

        vec![background]
    }
}
