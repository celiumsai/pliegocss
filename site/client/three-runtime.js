// SPDX-License-Identifier: Apache-2.0

// Keep the optional WebGL payload behind a route-level dynamic import while
// preserving Three.js tree shaking. Re-exporting only the scene primitives used
// by PliegoCSS prevents the browser from downloading the entire library.
export {
  BoxGeometry,
  CatmullRomCurve3,
  DoubleSide,
  Group,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Raycaster,
  Scene,
  TubeGeometry,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
