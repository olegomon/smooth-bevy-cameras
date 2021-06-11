use crate::{LookPolarity, OrbitTransform, OrbitTransformBundle, PolarDirection, Smoother};

use bevy::{
    app::prelude::*,
    ecs::{bundle::Bundle, prelude::*},
    input::{
        mouse::{MouseMotion, MouseWheel},
        prelude::*,
    },
    math::prelude::*,
    render::prelude::*,
    transform::components::Transform,
};
use serde::{Deserialize, Serialize};

pub struct UnrealCameraPlugin;

impl Plugin for UnrealCameraPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system(default_input_map.system())
            .add_system(control_system.system())
            .add_event::<ControlEvent>();
    }
}

#[derive(Bundle)]
pub struct UnrealCameraBundle {
    controller: UnrealCameraController,
    #[bundle]
    orbit_transform: OrbitTransformBundle,
    #[bundle]
    perspective: PerspectiveCameraBundle,
}

impl UnrealCameraBundle {
    pub fn new(
        controller: UnrealCameraController,
        mut perspective: PerspectiveCameraBundle,
        eye: Vec3,
        target: Vec3,
    ) -> Self {
        // Make sure the transform is consistent with the controller to start.
        perspective.transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);

        Self {
            controller,
            orbit_transform: OrbitTransformBundle {
                transform: OrbitTransform {
                    pivot: eye,
                    orbit: target,
                },
                polarity: LookPolarity::PivotLookAtOrbit,
                smoother: Smoother::new(controller.smoothing_weight),
            },
            perspective,
        }
    }
}

/// A camera controlled with the mouse in the same way as Unreal Engine's viewport controller.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct UnrealCameraController {
    pub enabled: bool,
    pub mouse_rotate_sensitivity: Vec2,
    pub mouse_translate_sensitivity: Vec2,
    pub trackpad_translate_sensitivity: Vec2,
    pub smoothing_weight: f32,
}

impl Default for UnrealCameraController {
    fn default() -> Self {
        Self {
            enabled: true,
            mouse_rotate_sensitivity: Vec2::splat(0.002),
            mouse_translate_sensitivity: Vec2::splat(0.1),
            trackpad_translate_sensitivity: Vec2::splat(0.1),
            smoothing_weight: 0.9,
        }
    }
}

pub enum ControlEvent {
    Locomotion(Vec2),
    Rotate(Vec2),
    Translate(Vec2),
}

pub fn default_input_map(
    mut events: EventWriter<ControlEvent>,
    mut mouse_wheel_reader: EventReader<MouseWheel>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mouse_buttons: Res<Input<MouseButton>>,
    controllers: Query<&UnrealCameraController>,
) {
    let controller = if let Some(controller) = controllers.iter().next() {
        controller
    } else {
        return;
    };
    let UnrealCameraController {
        enabled,
        mouse_translate_sensitivity,
        mouse_rotate_sensitivity,
        trackpad_translate_sensitivity,
        ..
    } = *controller;

    if !enabled {
        return;
    }

    let mut mouse_delta = Vec2::ZERO;
    for event in mouse_motion_events.iter() {
        mouse_delta += event.delta;
    }

    match (
        mouse_buttons.pressed(MouseButton::Left),
        mouse_buttons.pressed(MouseButton::Right),
    ) {
        (true, true) => {
            events.send(ControlEvent::Translate(
                mouse_translate_sensitivity * mouse_delta,
            ));
        }
        (true, false) => {
            events.send(ControlEvent::Locomotion(Vec2::new(
                mouse_rotate_sensitivity.x * mouse_delta.x,
                mouse_translate_sensitivity.y * mouse_delta.y,
            )));
        }
        (false, true) => {
            events.send(ControlEvent::Rotate(mouse_rotate_sensitivity * mouse_delta));
        }
        _ => (),
    }

    // On Mac, mouse wheel is the trackpad, treated the same as both mouse buttons down.
    let mut trackpad_delta = Vec2::ZERO;
    for event in mouse_wheel_reader.iter() {
        trackpad_delta.x += event.x;
        trackpad_delta.y += event.y;
    }
    events.send(ControlEvent::Translate(
        trackpad_translate_sensitivity * trackpad_delta,
    ));
}

pub fn control_system(
    mut events: EventReader<ControlEvent>,
    mut cameras: Query<(&UnrealCameraController, &mut OrbitTransform)>,
) {
    let (controller, mut transform) =
        if let Some((controller, transform)) = cameras.iter_mut().next() {
            (controller, transform)
        } else {
            return;
        };

    if controller.enabled {
        let look_vector = transform.pivot_to_orbit_direction();
        let mut polar_vector = PolarDirection::from_vector(look_vector);
        let forward_vector = Vec3::new(look_vector.x, 0.0, look_vector.z).normalize();

        let yaw_rot = Quat::from_axis_angle(Vec3::Y, polar_vector.get_yaw());
        let rot_x = yaw_rot * Vec3::X;
        let rot_y = yaw_rot * Vec3::Y;

        for event in events.iter() {
            match event {
                ControlEvent::Locomotion(delta) => {
                    // Translates forward/backward and rotates about the Y axis.
                    polar_vector.add_yaw(-delta.x);
                    transform.pivot -= delta.y * forward_vector;
                }
                ControlEvent::Rotate(delta) => {
                    // Rotates with pitch and yaw.
                    polar_vector.add_yaw(-delta.x);
                    polar_vector.add_pitch(-delta.y);
                }
                ControlEvent::Translate(delta) => {
                    // Translates up/down (Y) and left/right (X).
                    transform.pivot -= delta.x * rot_x + delta.y * rot_y;
                }
            }
        }

        polar_vector.assert_not_looking_up();

        transform.set_orbit_in_direction(polar_vector.unit_vector());
    } else {
        events.iter(); // Drop the events.
    }
}