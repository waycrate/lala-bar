#[derive(Copy, Clone, Default)]
pub enum SliderIndex {
    #[default]
    Balance,
    Left,
    Right,
}

impl SliderIndex {
    pub fn next(&self) -> Self {
        match self {
            SliderIndex::Balance => SliderIndex::Left,
            SliderIndex::Left => SliderIndex::Right,
            SliderIndex::Right => SliderIndex::Balance,
        }
    }
    pub fn pre(&self) -> Self {
        match self {
            SliderIndex::Balance => SliderIndex::Right,
            SliderIndex::Left => SliderIndex::Balance,
            SliderIndex::Right => SliderIndex::Left,
        }
    }
}
