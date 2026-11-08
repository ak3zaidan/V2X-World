/**
 * The actor material: a stock three.js Phong (near) or Lambert (far) material whose vertex shader
 * is taught four things, from one per-instance `vec4` (`iAnim`) and the per-vertex part codes of
 * `vehicle-models.ts`:
 *
 * 1. **Paint.** Only `ACTOR_PART.PAINT` vertices take the instance colour; trim, glass, tyres and
 *    lamp lenses keep their own. Before, the whole car was multiplied by the state colour, so the
 *    windows and the tyres of a benign car were grey and those of a revoked one purple.
 * 2. **Wheels.** `iAnim.x` is the wheel's rotation (distance rolled over the rolling radius) about
 *    its hub's lateral axis, and `iAnim.y` the steering angle of the front wheels about a vertical
 *    axis through the hub.
 * 3. **Limbs.** `iAnim.z` is the gait phase: legs swing about the hips in opposition, arms about the
 *    shoulders against the legs, with an amplitude that grows with walking speed and is zero
 *    standing still.
 * 4. **Lamps.** `iAnim.w` packs the stream's `lamps` byte (vwp-v1 §3.3.5) with a per-vehicle flash
 *    phase, the swing amplitude and the fade (`lamps + 256·phase + 16384·amplitude + 2²⁰·fade`),
 *    all small integers, which a 32-bit float carries exactly. Lamp parts add an emissive term: headlamps with low beam, tail
 *    lamps dim with low beam and bright with the brake, indicators flashing at 1.5 Hz (90 flashes
 *    a minute, inside SAE J590's 60–120), beacons alternating red and blue at 2 Hz (inside SAE J845's
 *    minimum of 75 flashes a minute). Brightness rises with the scene's darkness (`uNight`), as a
 *    lamp's contrast does.
 *
 * Everything is in the vertex shader, so it costs nothing per pixel beyond one varying, and the
 * cast shadow (off by default for actors) is the unanimated body.
 */

import { MeshLambertMaterial, MeshPhongMaterial, type Material } from "three";
import type { LodLevel } from "./types.js";

/** Uniforms every actor material shares; update them once a frame. */
export interface ActorUniforms {
  readonly uTime: { value: number };
  readonly uNight: { value: number };
}

export function makeActorUniforms(): ActorUniforms {
  return { uTime: { value: 0 }, uNight: { value: 0 } };
}

const VERTEX_PARS = /* glsl */ `
attribute float aPart;
attribute vec3 aPivot;
attribute vec4 iAnim;
uniform float uTime;
uniform float uNight;
varying vec3 vGlow;
varying float vSpec;
varying float vFade;
float vwpBit(float bits, float b) { return mod(floor(bits / b), 2.0); }
mat3 vwpRotY(float a) { float c = cos(a); float s = sin(a); return mat3(c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c); }
mat3 vwpRotZ(float a) { float c = cos(a); float s = sin(a); return mat3(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0); }
`;

const BEGIN_NORMAL = /* glsl */ `
#include <beginnormal_vertex>
float vwpPart = aPart;
float vwpPacked = iAnim.w;
float vwpLamps = mod(vwpPacked, 256.0);
float vwpPhase = mod(floor(vwpPacked / 256.0), 64.0) / 64.0;
float vwpAmp = mod(floor(vwpPacked / 16384.0), 64.0) / 63.0;
vFade = floor(vwpPacked / 1048576.0) / 15.0;
mat3 vwpRot = mat3(1.0);
if (vwpPart > 1.5 && vwpPart < 3.5) {
  vwpRot = vwpRotY(iAnim.x);
  if (vwpPart < 2.5) vwpRot = vwpRotZ(iAnim.y) * vwpRot;
} else if (vwpPart > 10.5 && vwpPart < 14.5) {
  float sw = sin(iAnim.z) * 0.42 * vwpAmp;
  if (vwpPart < 11.5) vwpRot = vwpRotY(sw);
  else if (vwpPart < 12.5) vwpRot = vwpRotY(-sw);
  else if (vwpPart < 13.5) vwpRot = vwpRotY(-0.8 * sw);
  else vwpRot = vwpRotY(0.8 * sw);
}
objectNormal = vwpRot * objectNormal;
`;

const BEGIN_VERTEX = /* glsl */ `
vec3 transformed = aPivot + vwpRot * (vec3(position) - aPivot);
vSpec = vwpPart > 15.5 ? 1.0 : (vwpPart < 0.5 ? 0.6 : 0.12);
vGlow = vec3(0.0);
float vwpNight = clamp(uNight, 0.0, 1.0);
float vwpBlink = step(0.5, fract((uTime + vwpPhase) * 1.5));
if (vwpPart > 3.5 && vwpPart < 4.5) {
  vGlow = vec3(1.0, 0.96, 0.88) * vwpBit(vwpLamps, 16.0) * mix(0.5, 2.4, vwpNight);
} else if (vwpPart > 4.5 && vwpPart < 5.5) {
  float brake = vwpBit(vwpLamps, 1.0);
  float tail = vwpBit(vwpLamps, 16.0);
  vGlow = vec3(1.0, 0.05, 0.03) * max(brake * 1.7, tail * 0.5) * mix(0.9, 1.7, vwpNight);
} else if (vwpPart > 5.5 && vwpPart < 7.5) {
  float hazard = vwpBit(vwpLamps, 8.0);
  float side = vwpPart < 6.5 ? vwpBit(vwpLamps, 2.0) : vwpBit(vwpLamps, 4.0);
  vGlow = vec3(1.0, 0.5, 0.04) * max(side, hazard) * vwpBlink * mix(1.6, 2.2, vwpNight);
} else if (vwpPart > 7.5 && vwpPart < 8.5) {
  vGlow = vec3(1.0, 1.0, 0.95) * vwpBit(vwpLamps, 32.0) * 1.4;
} else if (vwpPart > 8.5 && vwpPart < 10.5) {
  float on = vwpBit(vwpLamps, 64.0);
  float red = step(fract((uTime + vwpPhase) * 2.0), 0.5);
  float me = vwpPart < 9.5 ? red : 1.0 - red;
  vec3 hue = vwpPart < 9.5 ? vec3(1.0, 0.06, 0.05) : vec3(0.12, 0.3, 1.0);
  vGlow = hue * on * me * 2.6;
} else if (vwpPart > 14.5 && vwpPart < 15.5) {
  vGlow = vec3(1.0, 0.05, 0.03) * vwpBit(vwpLamps, 1.0) * 1.7;
}
`;

const COLOR_VERTEX = /* glsl */ `
#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR )
  vColor = vec4( 1.0 );
#endif
#ifdef USE_COLOR_ALPHA
  vColor *= color;
#elif defined( USE_COLOR )
  vColor.rgb *= color;
#endif
#ifdef USE_INSTANCING_COLOR
  if (aPart < 0.5) vColor.rgb *= instanceColor.rgb;
#endif
`;

const FRAGMENT_PARS = /* glsl */ `
varying vec3 vGlow;
varying float vSpec;
varying float vFade;
`;

/**
 * Screen-door fade: a 4×4 ordered-dither threshold (Bayer 1973) against the instance's fade, so an
 * actor appearing or leaving dissolves in over a few frames with no transparency sort — the whole
 * crowd stays in one opaque instanced draw.
 */
const FADE_FRAGMENT = /* glsl */ `
  if (vFade < 0.999) {
    vec2 q = mod(floor(gl_FragCoord.xy), 4.0);
    vec2 lo = mod(q, 2.0);
    vec2 hi = floor(q / 2.0);
    float bayer = 4.0 * mod(2.0 * lo.x + 3.0 * lo.y, 4.0) + mod(2.0 * hi.x + 3.0 * hi.y, 4.0);
    if ((bayer + 0.5) / 16.0 > vFade) discard;
  }
`;

/** Build the material for one LOD. `uniforms` is shared by every actor material. */
export function makeActorMaterial(lod: LodLevel, uniforms: ActorUniforms): Material {
  const material = lod === 2
    ? new MeshLambertMaterial({ vertexColors: true, flatShading: true, name: "actor-lod2" })
    : new MeshPhongMaterial({
      vertexColors: true, name: `actor-lod${lod}`, shininess: 48, specular: 0x3a3a3a, flatShading: lod === 1,
    });
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uNight = uniforms.uNight;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
      .replace("#include <color_vertex>", COLOR_VERTEX)
      .replace("#include <beginnormal_vertex>", BEGIN_NORMAL)
      .replace("#include <begin_vertex>", BEGIN_VERTEX);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace("#include <clipping_planes_fragment>", `#include <clipping_planes_fragment>\n${FADE_FRAGMENT}`)
      .replace("#include <emissivemap_fragment>", "#include <emissivemap_fragment>\n  totalEmissiveRadiance += vGlow;")
      .replace("#include <specularmap_fragment>", "float specularStrength = vSpec;");
  };
  material.customProgramCacheKey = () => `vwp-actor-${lod}`;
  return material;
}

/** The shader sources, for tests that check the patch points exist in the installed three.js. */
export const ACTOR_SHADER_PATCH_POINTS = [
  "#include <common>", "#include <color_vertex>", "#include <beginnormal_vertex>", "#include <begin_vertex>",
  "#include <emissivemap_fragment>", "#include <specularmap_fragment>", "#include <clipping_planes_fragment>",
] as const;

/**
 * Pack the per-instance `iAnim.w`: the lamps byte, a flash phase in 64ths, a swing amplitude in
 * 63rds and a fade in 15ths — 24 bits, which a 32-bit float carries exactly.
 */
export function packLamps(lamps: number, phase01: number, amplitude01: number, fade01 = 1): number {
  const ph = Math.max(0, Math.min(63, Math.floor(phase01 * 64)));
  const amp = Math.max(0, Math.min(63, Math.round(amplitude01 * 63)));
  const fade = Math.max(0, Math.min(15, Math.round(fade01 * 15)));
  return (lamps & 255) + 256 * ph + 16384 * amp + 1048576 * fade;
}
