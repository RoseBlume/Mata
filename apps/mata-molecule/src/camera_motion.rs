//! Frame-rate-independent motion for the 3D viewer.
//!
//! Key state is sampled once per frame. Velocities approach their targets
//! smoothly while a key is down and decay after release, without depending on
//! operating-system key-repeat events.

/// Multiply the current zoom without an artificial viewing limit. Only
/// saturate at the representable positive f32 range, so the renderer never
/// receives zero, infinity, or NaN. This is not a user-facing zoom cap.
pub fn multiply_zoom(zoom: &mut f32, factor: f32) -> bool {
    if !factor.is_finite() || factor <= 0.0 { return false; }
    let current = if zoom.is_finite() && *zoom > 0.0 { *zoom } else { 1.0 };
    // Compute in f64 so valid zoom steps don't overflow before saturation.
    let next = ((current as f64) * (factor as f64))
        .clamp(f32::MIN_POSITIVE as f64, f32::MAX as f64) as f32;
    let changed = *zoom != next;
    *zoom = next;
    changed
}

#[derive(Debug, Default)]
pub struct CameraMotion {
    pan_x_velocity: f32,
    pan_y_velocity: f32,
    zoom_velocity: f32,
}

impl CameraMotion {
    /// Units are fractions of the viewport height per second (pan) and
    /// logarithmic zoom units per second (zoom).
    const PAN_SPEED: f32 = 0.90;
    const ZOOM_SPEED: f32 = 1.50;
    const ACCELERATION: f32 = 11.0;
    const BRAKING: f32 = 15.0;
    const STOP_THRESHOLD: f32 = 0.001;

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `axes`: horizontal pan, vertical pan, and zoom, each in [-1, 1].
    /// Returns true while a visible update is needed (including deceleration).
    pub fn advance(
        &mut self,
        elapsed_seconds: f32,
        axes: (f32, f32, f32),
        pan: &mut (f32, f32),
        zoom: &mut f32,
    ) -> bool {
        if !elapsed_seconds.is_finite() || elapsed_seconds <= 0.0 {
            return false;
        }
        // Avoid jumping after a breakpoint, alt-tab, or blocked file dialog.
        let dt = elapsed_seconds.min(0.05);
        let (mut x, mut y, z) = axes;
        let planar_length = x.hypot(y);
        // Diagonals should not move faster than a single held direction.
        if planar_length > 1.0 {
            x /= planar_length;
            y /= planar_length;
        }
        let goal_x = x.clamp(-1.0, 1.0) * Self::PAN_SPEED;
        let goal_y = y.clamp(-1.0, 1.0) * Self::PAN_SPEED;
        // The user can keep holding Q/E at any magnification.
        let goal_z = if z.is_finite() { z.clamp(-1.0, 1.0) * Self::ZOOM_SPEED } else { 0.0 };

        fn ease(current: &mut f32, goal: f32, dt: f32, acceleration: f32, braking: f32, threshold: f32) {
            let response = if goal.abs() > f32::EPSILON { acceleration } else { braking };
            *current += (goal - *current) * (1.0 - (-response * dt).exp());
            if goal == 0.0 && current.abs() < threshold {
                *current = 0.0;
            }
        }

        ease(&mut self.pan_x_velocity, goal_x, dt, Self::ACCELERATION, Self::BRAKING, Self::STOP_THRESHOLD);
        ease(&mut self.pan_y_velocity, goal_y, dt, Self::ACCELERATION, Self::BRAKING, Self::STOP_THRESHOLD);
        ease(&mut self.zoom_velocity, goal_z, dt, Self::ACCELERATION, Self::BRAKING, Self::STOP_THRESHOLD);

        let mut changed = false;
        if self.pan_x_velocity != 0.0 || self.pan_y_velocity != 0.0 {
            pan.0 += self.pan_x_velocity * dt;
            pan.1 += self.pan_y_velocity * dt;
            changed = true;
        }
        if self.zoom_velocity != 0.0 {
            // Multiplicative zoom retains the same speed and easing at any scale.
            // Only redraw when f32 can represent an actual change.
            changed |= multiply_zoom(zoom, (self.zoom_velocity * dt).exp());
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holding_a_key_keeps_moving_without_key_repeat() {
        let mut camera = CameraMotion::default();
        let mut pan = (0.0, 0.0);
        let mut zoom = 1.0;
        for _ in 0..60 {
            assert!(camera.advance(1.0 / 60.0, (0.0, 1.0, 0.0), &mut pan, &mut zoom));
        }
        let after_one_second = pan.1;
        for _ in 0..60 {
            camera.advance(1.0 / 60.0, (0.0, 1.0, 0.0), &mut pan, &mut zoom);
        }
        assert!(after_one_second > 0.6);
        assert!(pan.1 > after_one_second + 0.8);
    }

    #[test]
    fn release_decelerates_and_eventually_stops() {
        let mut camera = CameraMotion::default();
        let mut pan = (0.0, 0.0);
        let mut zoom = 1.0;
        for _ in 0..30 {
            camera.advance(1.0 / 60.0, (1.0, 0.0, 0.0), &mut pan, &mut zoom);
        }
        let before_release = pan.0;
        for _ in 0..90 {
            camera.advance(1.0 / 60.0, (0.0, 0.0, 0.0), &mut pan, &mut zoom);
        }
        assert!(pan.0 > before_release); // soft deceleration, not instant stop
        assert!(!camera.advance(1.0 / 60.0, (0.0, 0.0, 0.0), &mut pan, &mut zoom));
    }

    #[test]
    fn diagonal_motion_is_normalized() {
        let mut a = CameraMotion::default();
        let mut b = CameraMotion::default();
        let (mut pa, mut pb) = ((0.0, 0.0), (0.0, 0.0));
        let (mut za, mut zb) = (1.0, 1.0);
        for _ in 0..60 {
            a.advance(1.0 / 60.0, (1.0, 0.0, 0.0), &mut pa, &mut za);
            b.advance(1.0 / 60.0, (1.0, 1.0, 0.0), &mut pb, &mut zb);
        }
        assert!((pa.0 - pb.0.hypot(pb.1)).abs() < 0.01);
    }

    #[test]
    fn held_zoom_passes_the_old_six_times_limit_and_reverses() {
        let mut camera = CameraMotion::default();
        let mut pan = (0.0, 0.0);
        let mut zoom = 1.0;
        camera.advance(1.0 / 60.0, (0.0, 0.0, 1.0), &mut pan, &mut zoom);
        assert!(zoom > 1.0 && zoom < 1.03);
        for _ in 0..240 {
            camera.advance(1.0 / 60.0, (0.0, 0.0, 1.0), &mut pan, &mut zoom);
        }
        assert!(zoom > 6.0, "keyboard zoom should not stop at 6x: {zoom}");
        let near = zoom;
        for _ in 0..120 {
            camera.advance(1.0 / 60.0, (0.0, 0.0, -1.0), &mut pan, &mut zoom);
        }
        assert!(zoom < near && zoom.is_finite());
    }

    #[test]
    fn zoom_out_passes_the_old_quarter_limit() {
        let mut camera = CameraMotion::default();
        let mut pan = (0.0, 0.0);
        let mut zoom = 1.0;
        for _ in 0..200 {
            camera.advance(1.0 / 60.0, (0.0, 0.0, -1.0), &mut pan, &mut zoom);
        }
        assert!(zoom < 0.25 && zoom > 0.0);
    }

    #[test]
    fn wheel_zoom_remains_positive_and_can_reverse_at_f32_limits() {
        let mut zoom = 6.0;
        assert!(multiply_zoom(&mut zoom, 1.09));
        assert!(zoom > 6.0);
        for _ in 0..2000 { multiply_zoom(&mut zoom, 1.09); }
        assert_eq!(zoom, f32::MAX);
        assert!(multiply_zoom(&mut zoom, 1.0 / 1.09));
        assert!(zoom < f32::MAX);
        zoom = 0.25;
        assert!(multiply_zoom(&mut zoom, 1.0 / 1.09));
        assert!(zoom < 0.25);
        for _ in 0..2000 { multiply_zoom(&mut zoom, 1.0 / 1.09); }
        assert_eq!(zoom, f32::MIN_POSITIVE);
        assert!(multiply_zoom(&mut zoom, 1.09));
    }

    #[test]
    fn large_frame_delta_is_capped() {
        let mut camera = CameraMotion::default();
        let mut pan = (0.0, 0.0);
        let mut zoom = 1.0;
        camera.advance(12.0, (1.0, 0.0, 0.0), &mut pan, &mut zoom);
        assert!(pan.0 < 0.05);
    }
}
