#version 330

#define M_PI 3.1415926535897932384626433832795

in vec4 gl_FragCoord;
in vec4 fragColor;
uniform vec3 u_blink_band;
uniform float u_time;
uniform vec2 u_resolution;
uniform vec2 u_top_left;
out vec4 finalColor;

void main()
{
    vec2 top_left = vec2(u_top_left.x, (u_resolution.y - u_top_left.y));
    vec2 offset = (top_left - gl_FragCoord.xy)/vec2(20, 20);
    float sp = ((offset.x + offset.y)/2 * M_PI) + u_time*4;
    float pct = sin(sp) * sin(sp);
	vec3 color = mix(fragColor.xyz, u_blink_band, (clamp(pct, 0.5, 1) * 2) - 1);
    finalColor = vec4(color, 1);
}