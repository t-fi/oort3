use crate::simulation::{Simulation, WORLD_SIZE};
use macroquad::math::{vec2, Vec2};
use macroquad::{camera, color, math, shapes, window};
use rapier2d_f64::dynamics::RigidBodyHandle;

pub fn render(camera_target: Vec2, zoom: f32, sim: &Simulation) {
    window::clear_background(color::BLACK);

    camera::set_camera(&camera::Camera2D {
        zoom: vec2(
            zoom,
            zoom * window::screen_width() / window::screen_height(),
        ),
        target: camera_target,
        ..Default::default()
    });

    let grid_size = 100.0;
    let n = 1 + (WORLD_SIZE as f32 / grid_size) as i32;
    for i in -(n / 2)..(n / 2 + 1) {
        shapes::draw_line(
            (i as f32) * grid_size,
            (-WORLD_SIZE as f32) / 2.0,
            (i as f32) * grid_size,
            (WORLD_SIZE as f32) / 2.0,
            1.0,
            color::GREEN,
        );
        shapes::draw_line(
            (-WORLD_SIZE as f32) / 2.0,
            (i as f32) * grid_size,
            (WORLD_SIZE as f32) / 2.0,
            (i as f32) * grid_size,
            1.0,
            color::GREEN,
        );
    }

    {
        let v = -WORLD_SIZE as f32 / 2.0;
        shapes::draw_line(-v, -v, v, -v, 1.0, color::RED);
        shapes::draw_line(-v, v, v, v, 1.0, color::RED);
        shapes::draw_line(-v, -v, -v, v, 1.0, color::RED);
        shapes::draw_line(v, -v, v, v, 1.0, color::RED);
    }

    for &index in sim.bullets.iter() {
        let body = sim.bodies.get(RigidBodyHandle(index)).unwrap();
        let x = body.position().translation.x as f32;
        let y = body.position().translation.y as f32;
        let vx = body.linvel().x as f32;
        let vy = body.linvel().y as f32;
        let dt = 2.0 / 60.0;
        shapes::draw_line(x, y, x - vx * dt, y - vy * dt, 1.0, color::ORANGE);
    }

    for &index in sim.ships.iter() {
        let body = sim.bodies.get(RigidBodyHandle(index)).unwrap();
        let x = body.position().translation.x as f32;
        let y = body.position().translation.y as f32;
        let h = body.position().rotation.angle() as f32;
        let matrix = math::Mat2::from_angle(h);
        let translation = vec2(x, y);

        let vertices = crate::model::ship()
            .iter()
            .map(|&v| translation + matrix.mul_vec2(v))
            .collect::<Vec<_>>();
        shapes::draw_triangle(vertices[0], vertices[1], vertices[2], color::YELLOW);
    }
}
