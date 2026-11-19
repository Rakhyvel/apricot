
#version 330 core

in vec2 v_ndc;

out vec4 color;

uniform vec3 u_sun_dir;
uniform vec3 u_camera_pos;
uniform mat4 u_inv_view_proj;

const vec3 MIE_COLOR = vec3(1.0, 0.75, 0.5);

// ---------------------------------------------------------
// ATMOSPHERIC COLOR MODEL
// Keep identical to the fog shader!
// ---------------------------------------------------------

vec3 get_horizon_color(float sun_height) {
    vec3 day = vec3(0.65, 0.80, 0.95);
    vec3 sunset = vec3(0.95, 0.55, 0.25);

    return mix(sunset, day, smoothstep(-0.12, 0.25, sun_height));
}

vec3 get_zenith_color(float sun_height) {
    vec3 day = vec3(0.08, 0.25, 0.55);
    vec3 sunset = vec3(0.25, 0.22, 0.28);

    return mix(sunset, day, smoothstep(-0.12, 0.25, sun_height));
}

vec3 get_atmosphere_color(vec3 view_dir, vec3 sun_dir) {
    float zenith = clamp(view_dir.y, 0.0, 1.0);

    vec3 horizon = get_horizon_color(sun_dir.y);
    vec3 zenith_color = get_zenith_color(sun_dir.y);

    float gradient = pow(zenith, 0.4);

    return mix(horizon, zenith_color, gradient);
}

// ---------------------------------------------------------
// MIE SCATTERING
// ---------------------------------------------------------

float mie_scattering(float cos_theta) {
    float g = 0.76;

    return (1.0 - g * g) /
        (4.0 * 3.14159 *
            pow(1.0 + g * g - 2.0 * g * cos_theta, 1.5));
}

// ---------------------------------------------------------
// MAIN
// ---------------------------------------------------------

void main() {
    // Extract out the world pos from the ndc and the fact we're on the far plane
    vec4 world_pos = u_inv_view_proj * vec4(v_ndc, 1.0, 1.0);

    // Get where we're looking at
    vec3 view_dir = normalize(world_pos.xyz / world_pos.w - u_camera_pos);

    // Normalize sun direction
    vec3 sun_dir = dot(u_sun_dir, u_sun_dir) > 0.0
        ? normalize(u_sun_dir) : normalize(vec3(0.4, 0.3, 0.6));

    float sun_height = sun_dir.y;

    // Base atmospheric gradient
    vec3 sky = get_atmosphere_color(view_dir, sun_dir);

    // Forward scattering around the sun
    float cos_theta = dot(view_dir, sun_dir);
    float mie_phase = mie_scattering(cos_theta);

    sky += MIE_COLOR * mie_phase *
            max(sun_height, 0.1) * 0.3;

    color = vec4(sky, 1.0);
}
