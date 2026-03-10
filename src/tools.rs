use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Tool {
    #[default]
    Select,
    Hand,
    Laser,
    Pen,
    Pencil,
    Highlighter,
    Eraser,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Diamond,
    Triangle,
    Polygon,
    Star,
    Text,
    Image,
}

impl Tool {
    pub const LEFT_TOOLBAR: [Tool; 17] = [
        Tool::Select,
        Tool::Hand,
        Tool::Laser,
        Tool::Pen,
        Tool::Pencil,
        Tool::Highlighter,
        Tool::Eraser,
        Tool::Line,
        Tool::Arrow,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Diamond,
        Tool::Triangle,
        Tool::Polygon,
        Tool::Star,
        Tool::Text,
        Tool::Image,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Hand => "Hand",
            Tool::Laser => "Laser",
            Tool::Pen => "Pen",
            Tool::Pencil => "Pencil",
            Tool::Highlighter => "Highlighter",
            Tool::Eraser => "Eraser",
            Tool::Line => "Line",
            Tool::Arrow => "Arrow",
            Tool::Rectangle => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Diamond => "Diamond",
            Tool::Triangle => "Triangle",
            Tool::Polygon => "Polygon",
            Tool::Star => "Star",
            Tool::Text => "Text",
            Tool::Image => "Image",
        }
    }

    pub fn hotkey(self) -> &'static str {
        match self {
            Tool::Select => "V",
            Tool::Hand => "H",
            Tool::Laser => "K",
            Tool::Pen => "P",
            Tool::Pencil => "N",
            Tool::Highlighter => "Y",
            Tool::Eraser => "E",
            Tool::Line => "L",
            Tool::Arrow => "A",
            Tool::Rectangle => "R",
            Tool::Ellipse => "O",
            Tool::Diamond => "D",
            Tool::Triangle => "T",
            Tool::Polygon => "G",
            Tool::Star => "S",
            Tool::Text => "X",
            Tool::Image => "I",
        }
    }

    pub fn is_freehand(self) -> bool {
        matches!(self, Tool::Pen | Tool::Pencil | Tool::Highlighter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Rotate,
}
