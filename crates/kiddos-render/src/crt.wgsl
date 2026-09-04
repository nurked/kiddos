// KidDOS CRT shader: letterbox to 4:3, barrel curvature, scanlines, a soft
// phosphor glow, vignette and rounded corners. `crt == 0` draws it flat.

struct Params {
    screen_size: vec2<f32>,
    tex_size: vec2<f32>,
    time: f32,
    crt: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var grid_tex: texture_2d<f32>;
@group(0) @binding(1) var grid_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // one big triangle covering the screen
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var out: VsOut;
    out.pos = vec4(p[i], 0.0, 1.0);
    out.uv = vec2(p[i].x * 0.5 + 0.5, 0.5 - p[i].y * 0.5);
    return out;
}

fn curve(uv: vec2<f32>) -> vec2<f32> {
    var c = uv * 2.0 - 1.0;
    let r = c * c;
    c = c + c * r.yx * 0.06 + c * r * 0.03;
    return c * 0.5 + 0.5;
}

fn sample_grid(uv: vec2<f32>) -> vec3<f32> {
    // nearest-ish sampling: snap to texel centers so glyphs stay crisp
    let t = (floor(uv * params.tex_size) + 0.5) / params.tex_size;
    return textureSample(grid_tex, grid_sampler, t).rgb;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // letterbox: the 640x400-ish grid is shown at 4:3 like a real monitor
    let target_aspect = 4.0 / 3.0;
    let screen_aspect = params.screen_size.x / params.screen_size.y;
    var uv = in.uv;
    if (screen_aspect > target_aspect) {
        let w = target_aspect / screen_aspect;
        uv.x = (uv.x - (1.0 - w) * 0.5) / w;
    } else {
        let h = screen_aspect / target_aspect;
        uv.y = (uv.y - (1.0 - h) * 0.5) / h;
    }
    if (params.crt < 0.5) {
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4(0.0, 0.0, 0.0, 1.0);
        }
        let t = (floor(uv * params.tex_size) + 0.5) / params.tex_size;
        return vec4(textureSample(grid_tex, grid_sampler, t).rgb, 1.0);
    }
    // screen → curved glass; [0,1] is the glass, outside is the bezel
    let cuv = curve(uv);
    if (cuv.x < 0.0 || cuv.x > 1.0 || cuv.y < 0.0 || cuv.y > 1.0) {
        return vec4(0.02, 0.02, 0.02, 1.0);
    }
    // rounded corners of the glass (radius 2%), computed as a rounded-rect
    // distance so only the corners are trimmed, never the edges
    let radius = 0.02;
    let q = abs(cuv - 0.5) - vec2(0.5 - radius);
    let corner_dist = length(max(q, vec2(0.0))) - radius;
    if (corner_dist > 0.0) {
        return vec4(0.02, 0.02, 0.02, 1.0);
    }
    // the text sits inside a border on the glass, like a real terminal's
    // overscan, so every cell is fully lit
    let tuv = (cuv - 0.5) / vec2(0.955, 0.94) + 0.5;
    var col = vec3(0.0);
    if (tuv.x >= 0.0 && tuv.x <= 1.0 && tuv.y >= 0.0 && tuv.y <= 1.0) {
        col = sample_grid(tuv);
        // glow: cheap 4-tap blur added on top
        let px = 1.0 / params.tex_size;
        let glow = textureSample(grid_tex, grid_sampler, tuv + vec2(px.x, 0.0)).rgb
                 + textureSample(grid_tex, grid_sampler, tuv - vec2(px.x, 0.0)).rgb
                 + textureSample(grid_tex, grid_sampler, tuv + vec2(0.0, px.y)).rgb
                 + textureSample(grid_tex, grid_sampler, tuv - vec2(0.0, px.y)).rgb;
        col = col + glow * 0.09;
        // scanlines: one dark line per texel row
        let line = sin(tuv.y * params.tex_size.y * 3.14159);
        col = col * (0.82 + 0.18 * line * line);
    }
    // subtle RGB stripe mask
    let mask_x = i32(floor(in.uv.x * params.screen_size.x)) % 3;
    var mask = vec3(0.94, 0.94, 0.94);
    if (mask_x == 0) { mask.r = 1.06; } else if (mask_x == 1) { mask.g = 1.06; } else { mask.b = 1.06; }
    col = col * mask;
    // mild vignette
    let d = (cuv - 0.5) * 2.0;
    col = col * (1.0 - dot(d, d) * 0.12);
    // very slight flicker so it feels alive
    col = col * (0.985 + 0.015 * sin(params.time * 60.0));
    return vec4(col, 1.0);
}
