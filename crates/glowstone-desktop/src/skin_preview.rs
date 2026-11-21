use std::sync::Arc;

use iced::widget::canvas;
use iced::{Color, Point, Rectangle};

use crate::store::SkinModel;

#[derive(Debug, Clone)]
pub struct SkinPreview {
    texture: Arc<Vec<[u8; 4]>>,
    model: SkinModel,
    angle: f32,
}

impl SkinPreview {
    pub fn new(texture: Arc<Vec<[u8; 4]>>, model: SkinModel, angle: f32) -> Self {
        Self { texture, model, angle }
    }
}

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Copy)]
struct Tile {
    depth: f32,
    points: [Point; 4],
    color: Color,
}

#[derive(Debug, Clone, Copy)]
struct CubeUv {
    front: (u32, u32),
    side: (u32, u32),
    top: (u32, u32),
}

impl<Message> canvas::Program<Message> for SkinPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&background, Color::from_rgb8(17, 25, 41));

        if self.texture.len() < 64 * 32 {
            return vec![frame.into_geometry()];
        }

        let texture_height = (self.texture.len() / 64).min(64) as u32;
        let mut tiles = Vec::with_capacity(2600);
        let (width, height) = (bounds.width, bounds.height);
        let scale = (width / 23.0).min(height / 36.0) * 0.88;
        let center = Point::new(width * 0.5, height * 0.88);
        let yaw = 0.58 + self.angle;

        let head_uv = CubeUv {
            front: (8, 8),
            side: (0, 8),
            top: (8, 0),
        };
        let body_uv = CubeUv {
            front: (20, 20),
            side: (16, 20),
            top: (20, 16),
        };
        let right_arm_uv = CubeUv {
            front: (44, 20),
            side: (40, 20),
            top: (44, 16),
        };
        let left_arm_uv = if texture_height >= 64 {
            CubeUv { front: (36, 52), side: (32, 52), top: (36, 48) }
        } else {
            right_arm_uv
        };
        let right_leg_uv = CubeUv {
            front: (4, 20),
            side: (0, 20),
            top: (4, 16),
        };
        let left_leg_uv = if texture_height >= 64 {
            CubeUv { front: (20, 52), side: (16, 52), top: (20, 48) }
        } else {
            right_leg_uv
        };

        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: -4.0, y: 24.0, z: -4.0 },
            Vec3 { x: 4.0, y: 32.0, z: 4.0 },
            head_uv, (8, 8), false,
        );
        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: -4.0, y: 12.0, z: -2.0 },
            Vec3 { x: 4.0, y: 24.0, z: 2.0 },
            body_uv, (8, 12), false,
        );

        let arm_width = if self.model == SkinModel::Slim { 3.0 } else { 4.0 };
        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: -8.0, y: 12.0, z: -2.0 },
            Vec3 { x: -8.0 + arm_width, y: 24.0, z: 2.0 },
            left_arm_uv, (arm_width as u32, 12), false,
        );
        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: 4.0, y: 12.0, z: -2.0 },
            Vec3 { x: 4.0 + arm_width, y: 24.0, z: 2.0 },
            right_arm_uv, (arm_width as u32, 12), false,
        );

        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: -4.0, y: 0.0, z: -2.0 },
            Vec3 { x: 0.0, y: 12.0, z: 2.0 },
            left_leg_uv, (4, 12), false,
        );
        add_cube(
            &mut tiles, &self.texture, yaw, scale, center,
            Vec3 { x: 0.0, y: 0.0, z: -2.0 },
            Vec3 { x: 4.0, y: 12.0, z: 2.0 },
            right_leg_uv, (4, 12), false,
        );

        tiles.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        for tile in tiles {
            let mut builder = canvas::Path::builder();
            builder.move_to(tile.points[0]);
            builder.line_to(tile.points[1]);
            builder.line_to(tile.points[2]);
            builder.line_to(tile.points[3]);
            builder.close();
            frame.fill(&builder.build(), tile.color);
        }

        vec![frame.into_geometry()]
    }
}

fn add_cube(
    tiles: &mut Vec<Tile>,
    texture: &[[u8; 4]],
    yaw: f32,
    scale: f32,
    center: Point,
    min: Vec3,
    max: Vec3,
    uv: CubeUv,
    face_size: (u32, u32),
    _overlay: bool,
) {
    let (face_width, face_height) = face_size;
    add_front_face(
        tiles, texture, uv.front, face_width, face_height,
        min, max, max.z, yaw, scale, center, 1.0,
    );
    add_right_face(
        tiles, texture, uv.side, max.y as u32 - min.y as u32,
        (max.z - min.z) as u32, min, max, yaw, scale, center, 0.76,
    );
    add_top_face(
        tiles, texture, uv.top, face_width, (max.z - min.z) as u32,
        min, max, yaw, scale, center, 0.92,
    );
}

fn add_front_face(
    tiles: &mut Vec<Tile>,
    texture: &[[u8; 4]],
    uv: (u32, u32),
    face_width: u32,
    face_height: u32,
    min: Vec3,
    max: Vec3,
    z: f32,
    yaw: f32,
    scale: f32,
    center: Point,
    shade: f32,
) {
    for row in 0..face_height {
        for column in 0..face_width {
            let color = texel(texture, uv.0 + column, uv.1 + row, shade);
            if color.a < 0.02 { continue; }
            let x0 = min.x + (max.x - min.x) * column as f32 / face_width as f32;
            let x1 = min.x + (max.x - min.x) * (column + 1) as f32 / face_width as f32;
            let y1 = max.y - (max.y - min.y) * row as f32 / face_height as f32;
            let y0 = max.y - (max.y - min.y) * (row + 1) as f32 / face_height as f32;
            push_tile(tiles, [
                Vec3 { x: x0, y: y1, z },
                Vec3 { x: x1, y: y1, z },
                Vec3 { x: x1, y: y0, z },
                Vec3 { x: x0, y: y0, z },
            ], color, yaw, scale, center);
        }
    }
}

fn add_right_face(
    tiles: &mut Vec<Tile>,
    texture: &[[u8; 4]],
    uv: (u32, u32),
    face_height: u32,
    face_depth: u32,
    min: Vec3,
    max: Vec3,
    yaw: f32,
    scale: f32,
    center: Point,
    shade: f32,
) {
    for row in 0..face_height {
        for column in 0..face_depth {
            let color = texel(texture, uv.0 + column, uv.1 + row, shade);
            if color.a < 0.02 { continue; }
            let z0 = max.z - (max.z - min.z) * column as f32 / face_depth as f32;
            let z1 = max.z - (max.z - min.z) * (column + 1) as f32 / face_depth as f32;
            let y1 = max.y - (max.y - min.y) * row as f32 / face_height as f32;
            let y0 = max.y - (max.y - min.y) * (row + 1) as f32 / face_height as f32;
            push_tile(tiles, [
                Vec3 { x: max.x, y: y1, z: z0 },
                Vec3 { x: max.x, y: y1, z: z1 },
                Vec3 { x: max.x, y: y0, z: z1 },
                Vec3 { x: max.x, y: y0, z: z0 },
            ], color, yaw, scale, center);
        }
    }
}

fn add_top_face(
    tiles: &mut Vec<Tile>,
    texture: &[[u8; 4]],
    uv: (u32, u32),
    face_width: u32,
    face_depth: u32,
    min: Vec3,
    max: Vec3,
    yaw: f32,
    scale: f32,
    center: Point,
    shade: f32,
) {
    for row in 0..face_depth {
        for column in 0..face_width {
            let color = texel(texture, uv.0 + column, uv.1 + row, shade);
            if color.a < 0.02 { continue; }
            let x0 = min.x + (max.x - min.x) * column as f32 / face_width as f32;
            let x1 = min.x + (max.x - min.x) * (column + 1) as f32 / face_width as f32;
            let z0 = max.z - (max.z - min.z) * row as f32 / face_depth as f32;
            let z1 = max.z - (max.z - min.z) * (row + 1) as f32 / face_depth as f32;
            push_tile(tiles, [
                Vec3 { x: x0, y: max.y, z: z0 },
                Vec3 { x: x1, y: max.y, z: z0 },
                Vec3 { x: x1, y: max.y, z: z1 },
                Vec3 { x: x0, y: max.y, z: z1 },
            ], color, yaw, scale, center);
        }
    }
}

fn push_tile(
    tiles: &mut Vec<Tile>,
    vertices: [Vec3; 4],
    color: Color,
    yaw: f32,
    scale: f32,
    center: Point,
) {
    let mut depth = 0.0;
    let mut points = [Point::ORIGIN; 4];
    for (index, vertex) in vertices.into_iter().enumerate() {
        let (point, camera_depth) = project(vertex, yaw, scale, center);
        points[index] = point;
        depth += camera_depth * 0.25;
    }
    tiles.push(Tile { depth, points, color });
}

fn project(vertex: Vec3, yaw: f32, scale: f32, center: Point) -> (Point, f32) {
    let (sin, cos) = yaw.sin_cos();
    let camera_x = vertex.x * cos + vertex.z * sin;
    let camera_depth = -vertex.x * sin + vertex.z * cos;
    let point = Point::new(
        center.x + camera_x * scale,
        center.y - vertex.y * scale - camera_depth * scale * 0.34,
    );
    (point, camera_depth)
}

fn texel(texture: &[[u8; 4]], x: u32, y: u32, shade: f32) -> Color {
    if x >= 64 || y >= 64 {
        return Color::TRANSPARENT;
    }
    let Some(pixel) = texture.get((y * 64 + x) as usize) else {
        return Color::TRANSPARENT;
    };
    let alpha = pixel[3] as f32 / 255.0;
    Color::from_rgba(
        (pixel[0] as f32 / 255.0 * shade).min(1.0),
        (pixel[1] as f32 / 255.0 * shade).min(1.0),
        (pixel[2] as f32 / 255.0 * shade).min(1.0),
        alpha,
    )
}
