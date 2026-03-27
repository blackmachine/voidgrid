#version 330

in vec2 fragTexCoord;
in vec4 fragColor;

uniform sampler2D texture0;    // source / previous mip
uniform vec2 texelSize;        // 1.0 / current source resolution
uniform int uMode;             // 0=prefilter, 1=downsample, 2=upsample, 3=composite

// --- Prefilter uniforms ---
uniform float uGamma;          // pseudo-linearization gamma (~2.0)
uniform float uBrightBoost;    // brightness multiplier (~1.5-3.0)
uniform float uThreshold;      // bloom threshold (~0.5)
uniform float uKnee;           // soft threshold knee (~0.2)
uniform float uSatStart;       // desaturation start luma (~0.6)
uniform float uSatEnd;         // desaturation end luma (~1.0)
uniform float uDesatStrength;  // desaturation strength (~0.5)

// --- Upsample uniforms ---
uniform float uSampleScale;    // tent filter radius (~1.0)

// --- Composite uniforms ---
uniform float uIntensity;      // bloom mix intensity (~1.0)
uniform float uBloomGamma;     // power curve on bloom before composite (~1.0-3.0)
uniform float uBloomSaturation; // saturation of bloom layer (~1.0)

out vec4 finalColor;

float luminance(vec3 c) {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// ============================================================================
// Mode 0: Prefilter — pseudo-linearize + threshold + 13-tap downsample
// ============================================================================

vec3 pseudoLinearize(vec3 srgb) {
    // sRGB → pseudo-linear with brightness boost
    vec3 lin = pow(srgb, vec3(uGamma)) * uBrightBoost;

    // Desaturation by brightness (bright pixels → white glow)
    float luma = luminance(lin);
    float desat = smoothstep(uSatStart, uSatEnd, luma);
    lin = mix(lin, vec3(luma), desat * uDesatStrength);

    return lin;
}

vec3 thresholdFilter(vec3 color) {
    float brightness = max(color.r, max(color.g, color.b));
    float soft = brightness - uThreshold + uKnee;
    soft = clamp(soft, 0.0, 2.0 * uKnee);
    soft = soft * soft / (4.0 * uKnee + 0.0001);
    float contrib = max(soft, brightness - uThreshold) / max(brightness, 0.0001);
    return color * clamp(contrib, 0.0, 1.0);
}

// 13-tap downsample [Jimenez14]
// . . . . . . .
// . A . B . C .
// . . D . E . .
// . F . G . H .
// . . I . J . .
// . K . L . M .
// . . . . . . .
vec4 downsample13Tap(sampler2D src, vec2 uv, vec2 ts) {
    vec4 A = texture(src, uv + ts * vec2(-1.0, -1.0));
    vec4 B = texture(src, uv + ts * vec2( 0.0, -1.0));
    vec4 C = texture(src, uv + ts * vec2( 1.0, -1.0));
    vec4 D = texture(src, uv + ts * vec2(-0.5, -0.5));
    vec4 E = texture(src, uv + ts * vec2( 0.5, -0.5));
    vec4 F = texture(src, uv + ts * vec2(-1.0,  0.0));
    vec4 G = texture(src, uv);
    vec4 H = texture(src, uv + ts * vec2( 1.0,  0.0));
    vec4 I = texture(src, uv + ts * vec2(-0.5,  0.5));
    vec4 J = texture(src, uv + ts * vec2( 0.5,  0.5));
    vec4 K = texture(src, uv + ts * vec2(-1.0,  1.0));
    vec4 L = texture(src, uv + ts * vec2( 0.0,  1.0));
    vec4 M = texture(src, uv + ts * vec2( 1.0,  1.0));

    vec2 div = (1.0 / 4.0) * vec2(0.5, 0.125);

    vec4 o =  (D + E + I + J) * div.x;
         o += (A + B + G + F) * div.y;
         o += (B + C + H + G) * div.y;
         o += (F + G + L + K) * div.y;
         o += (G + H + M + L) * div.y;

    return o;
}

// ============================================================================
// Mode 2: 9-tap tent upsample [Jimenez14]
// ============================================================================

vec4 upsampleTent(sampler2D src, vec2 uv, vec2 ts) {
    vec4 d = ts.xyxy * vec4(1.0, 1.0, -1.0, 0.0) * uSampleScale;

    vec4 s;
    s =  texture(src, uv - d.xy);
    s += texture(src, uv - d.wy) * 2.0;
    s += texture(src, uv - d.zy);

    s += texture(src, uv + d.zw) * 2.0;
    s += texture(src, uv       ) * 4.0;
    s += texture(src, uv + d.xw) * 2.0;

    s += texture(src, uv + d.zy);
    s += texture(src, uv + d.wy) * 2.0;
    s += texture(src, uv + d.xy);

    return s * (1.0 / 16.0);
}

// ============================================================================
// Main
// ============================================================================

void main() {
    if (uMode == 0) {
        // Prefilter: linearize each tap, then 13-tap downsample, then threshold
        vec4 ds = downsample13Tap(texture0, fragTexCoord, texelSize);
        vec3 lin = pseudoLinearize(ds.rgb);
        vec3 filtered = thresholdFilter(lin);
        finalColor = vec4(filtered, ds.a);

    } else if (uMode == 1) {
        // Downsample: 13-tap (already in pseudo-linear space)
        finalColor = downsample13Tap(texture0, fragTexCoord, texelSize);

    } else if (uMode == 2) {
        // Upsample: 9-tap tent filter
        finalColor = upsampleTent(texture0, fragTexCoord, texelSize);

    } else if (uMode == 3) {
        // Composite pass 2: bloom layer with intensity (drawn additively)
        // texture0 = mip[0] bloom content
        vec3 bloom = texture(texture0, fragTexCoord).rgb;

        // Power curve: dims weak bloom nonlinearly, preserves bright peaks
        bloom = pow(bloom, vec3(uBloomGamma));

        // Saturation control on bloom layer
        float luma = luminance(bloom);
        bloom = mix(vec3(luma), bloom, uBloomSaturation);

        finalColor = vec4(bloom * uIntensity, 1.0);
    }
}
