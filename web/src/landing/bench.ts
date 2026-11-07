// The landing page's 3D bench: QDay's printed parts laid out on a cutting mat, assembled by scroll
// progress into the device, which then powers on and signs. Coordinates are the case's own, in mm
// (hardware/case/qday-case.scad): X right, Y toward the USB end, Z out of the screen. The device
// group turns that frame so Z points up off the mat.
import * as THREE from "three";
import { STLLoader } from "three/examples/jsm/loaders/STLLoader.js";

export interface BenchLabel {
  id: string;
  element: HTMLElement;
}

export interface BenchOptions {
  canvas: HTMLCanvasElement;
  labels: BenchLabel[];
  /** Where the scene's centre sits on the canvas at a given progress, each axis from 0 to 1. */
  focus: (progress: number) => { x: number; y: number };
  reducedMotion: boolean;
  onReady?: () => void;
  onError?: (error: unknown) => void;
}

export interface Bench {
  setProgress(progress: number): void;
  dispose(): void;
}

type V3 = [number, number, number];

const MAT_Z = -1.2; // The tray's underside: the mat surface, in case coordinates.

// Obsidian Chrome only: alabaster PLA, onyx and blue slate for the electronics.
const COLOURS = {
  pla: 0xe5e4e2, // Alabaster Grey
  board: 0x1a1d21, // onyx, lifted so it reads as a board
  shield: 0xa9b1b8, // alabaster through slate
  pcb: 0x3b4a56, // deep blue slate
  glass: 0x0a0a0a, // Onyx
  stripboard: 0x536878, // Blue Slate
  switch: 0x141517,
  cap: 0xe5e4e2,
  breadboard: 0xd9dadb,
  spacer: 0x536878,
  stylus: 0xe5e4e2,
};

/** One part: where it sits on the bench, when it travels to its seat, and how high it arcs. */
interface PartSpec {
  id: string;
  build: () => THREE.Object3D | Promise<THREE.Object3D>;
  bench: { x: number; y: number; turn: number };
  window: [number, number];
  lift: number;
}

const ease = (t: number) => (t <= 0 ? 0 : t >= 1 ? 1 : t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
const clamp01 = (t: number) => Math.min(1, Math.max(0, t));
const span = (p: number, a: number, b: number) => clamp01((p - a) / (b - a));

function box(x0: number, x1: number, y0: number, y1: number, z0: number, z1: number, colour: number, rough = 0.7) {
  const mesh = new THREE.Mesh(
    new THREE.BoxGeometry(x1 - x0, y1 - y0, z1 - z0),
    new THREE.MeshStandardMaterial({ color: colour, roughness: rough, metalness: 0.05 }),
  );
  mesh.position.set((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  return mesh;
}

function cylinder(x: number, y: number, d: number, z0: number, z1: number, colour: number) {
  const mesh = new THREE.Mesh(
    new THREE.CylinderGeometry(d / 2, d / 2, z1 - z0, 40),
    new THREE.MeshStandardMaterial({ color: colour, roughness: 0.45, metalness: 0.1 }),
  );
  mesh.rotation.x = Math.PI / 2; // Three's cylinders stand on Y; the case's stand on Z.
  mesh.position.set(x, y, (z0 + z1) / 2);
  mesh.castShadow = true;
  return mesh;
}

const plaMaterial = () => new THREE.MeshStandardMaterial({ color: COLOURS.pla, roughness: 0.62, metalness: 0 });

async function loadStl(url: string): Promise<THREE.BufferGeometry> {
  const geometry = await new STLLoader().loadAsync(url);
  geometry.computeVertexNormals();
  return geometry;
}

/** The laser marking on the module's shield, as a transparent texture. */
function markingTexture(): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 512;
  const g = canvas.getContext("2d")!;
  g.fillStyle = "rgb(36 41 46 / 0.82)";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.font = "600 50px ui-monospace, Menlo, monospace";
  g.fillText("ESP32-S3", 256, 196);
  g.font = "500 34px ui-monospace, Menlo, monospace";
  g.fillText("WROOM-1", 256, 262);
  g.fillText("N16R8", 256, 318);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

/**
 * The YD-ESP32-S3 board. The chip that holds the keys and signs is the ESP32-S3 under the
 * WROOM-1 module's shield, at the far end from the two USB-C ports, its PCB antenna outward.
 */
function esp32Board(): THREE.Group {
  const group = new THREE.Group();
  group.add(box(-6.1, 22.5, 16.08, 80.48, 11, 12.6, COLOURS.board, 0.6));
  // The two USB-C ports, inside the case model's envelope for them.
  group.add(box(-0.6, 7, 73.5, 81.5, 12.6, 15.8, COLOURS.shield, 0.35));
  group.add(box(8.4, 16, 73.5, 81.5, 12.6, 15.8, COLOURS.shield, 0.35));
  // The module: 18 x 25.5 x 3.1 mm, its shield can and the meandered antenna on its bare end.
  group.add(box(-0.8, 17.2, 16.6, 42.1, 12.6, 13.4, COLOURS.pcb, 0.55));
  group.add(box(-0.1, 16.5, 23.2, 41.5, 13.4, 15.7, COLOURS.shield, 0.3));
  for (let i = 0; i < 7; i++) {
    const x = 1.2 + i * 2.2;
    group.add(box(x, x + 0.6, 17.8, 21.6, 13.4, 13.48, COLOURS.shield, 0.4));
    const y = i % 2 ? 17.8 : 21;
    if (i < 6) group.add(box(x, x + 2.8, y, y + 0.6, 13.4, 13.48, COLOURS.shield, 0.4));
  }
  const marking = new THREE.Mesh(
    new THREE.PlaneGeometry(15, 15),
    new THREE.MeshStandardMaterial({ map: markingTexture(), transparent: true, roughness: 0.5 }),
  );
  marking.position.set(8.2, 32.35, 15.72);
  group.add(marking);
  // BOOT and RST beside the ports.
  for (const x of [1.2, 11.2]) {
    group.add(box(x, x + 4, 66, 69.6, 12.6, 14.1, COLOURS.switch, 0.5));
    group.add(box(x + 1.1, x + 2.9, 66.9, 68.7, 14.1, 14.6, COLOURS.shield, 0.4));
  }
  // The two header rows' solder joints, 22 pins each at 2.54 mm.
  const joints = new THREE.InstancedMesh(
    new THREE.CylinderGeometry(0.55, 0.7, 0.7, 10),
    new THREE.MeshStandardMaterial({ color: COLOURS.shield, roughness: 0.35, metalness: 0.1 }),
    44,
  );
  const place = new THREE.Object3D();
  place.rotation.x = Math.PI / 2;
  for (let i = 0; i < 44; i++) {
    place.position.set(i < 22 ? -3.23 : 19.63, 21.6 + (i % 22) * 2.54, 12.95);
    place.updateMatrix();
    joints.setMatrixAt(i, place.matrix);
  }
  joints.castShadow = true;
  group.add(joints);
  return group;
}

/** The parts, in assembly order, with their places on the bench. */
function partSpecs(): PartSpec[] {
  return [
    {
      id: "tray",
      build: async () => {
        // tray.stl is turned to print floor down; undo that to get case coordinates.
        const mesh = new THREE.Mesh(await loadStl("/landing/models/tray.stl"), plaMaterial());
        mesh.position.set(-16.3, -2.3, -1.2);
        mesh.castShadow = mesh.receiveShadow = true;
        return mesh;
      },
      bench: { x: -30, y: 150, turn: 0 },
      window: [0.08, 0.17],
      lift: 18,
    },
    {
      id: "breadboard",
      build: () => box(0, 43.8, 0, 82.5, 0, 8.5, COLOURS.breadboard, 0.85),
      bench: { x: -96, y: 22, turn: 0 },
      window: [0.13, 0.22],
      lift: 44,
    },
    {
      id: "spacer",
      build: () => box(45.4, 108.9, 9.515, 24.755, 0, 5, COLOURS.spacer, 0.8),
      bench: { x: 112, y: 4, turn: 0 },
      window: [0.17, 0.25],
      lift: 44,
    },
    {
      id: "esp32",
      build: esp32Board,
      bench: { x: -47, y: 22, turn: 0 },
      window: [0.21, 0.29],
      lift: 50,
    },
    {
      id: "screen",
      build: () => {
        const group = new THREE.Group();
        group.add(box(31.3, 117.3, 34.03, 84.03, 11, 12.6, COLOURS.pcb, 0.6));
        group.add(box(40.8, 110.8, 34.03, 84.03, 12.6, 18.6, COLOURS.glass, 0.25));
        return group;
      },
      bench: { x: 22, y: 30, turn: 0 },
      window: [0.25, 0.33],
      lift: 54,
    },
    {
      id: "keys",
      build: () => {
        const group = new THREE.Group();
        group.add(box(44.28, 107.78, 9.515, 24.755, 10.2, 11.8, COLOURS.stripboard, 0.75));
        for (const [i, x] of [54.44, 69.68, 84.92, 100.16].entries()) {
          group.add(box(x - 6, x + 6, 15.865 - 6, 15.865 + 6, 11.8, 15.4, COLOURS.switch, 0.5));
          const cap = cylinder(x, 15.865, 6.8, 15.4, 23.8, COLOURS.cap);
          cap.name = `cap-${i + 1}`;
          group.add(cap);
        }
        return group;
      },
      bench: { x: 112, y: 38, turn: 0 },
      window: [0.29, 0.37],
      lift: 56,
    },
    {
      id: "stylus",
      build: () => {
        const group = new THREE.Group();
        const body = cylinder(-12.3, 5.73 + 75 / 2, 5, 0, 75, COLOURS.stylus);
        // A cylinder along Y: stand it on Y instead of Z, at the stylus's height in the case.
        body.rotation.x = 0;
        body.position.set(-12.3, 5.73 + 75 / 2, 11);
        group.add(body);
        group.add(box(-14.8, -9.8, 80.73, 93.73, 5.5, 14.5, COLOURS.stylus, 0.5));
        return group;
      },
      bench: { x: 112, y: -24, turn: Math.PI / 2 },
      window: [0.33, 0.4],
      lift: 40,
    },
    {
      id: "bezel",
      build: async () => {
        // bezel.stl is turned face down for printing: rotate it back over X and move it home.
        const mesh = new THREE.Mesh(await loadStl("/landing/models/bezel.stl"), plaMaterial());
        mesh.rotation.x = Math.PI;
        mesh.position.set(-16.3, 86.33, 22.3);
        mesh.castShadow = mesh.receiveShadow = true;
        return mesh;
      },
      bench: { x: 126, y: 150, turn: 0 },
      window: [0.37, 0.47],
      lift: 70,
    },
  ];
}

/** A part wrapped in a pivot at its own centre, so it can turn in place on the bench. */
interface Placed {
  spec: PartSpec;
  pivot: THREE.Group;
  home: THREE.Vector3; // The pivot's position when assembled.
  benchAt: THREE.Vector3; // The pivot's position on the bench.
  label: THREE.Vector3; // Where its label sits on the bench: just past the part's far top edge.
}

// Camera keyframes: [progress, target (case coordinates), offset from target (case coordinates)].
const SHOTS: [number, V3, V3][] = [
  [0.0, [40, 72, 0], [0, -310, 485]],
  [0.08, [40, 72, 0], [0, -310, 485]],
  // Hold the bench framing while the parts fly, so none of them leaves the frame; close in as the bezel lands.
  [0.36, [46, 64, 4], [40, -300, 450]],
  [0.47, [52, 42, 12], [185, -232, 218]],
  [0.58, [70, 44, 16], [86, -200, 236]],
  [0.84, [72, 46, 16], [50, -186, 228]],
  [1.0, [54, 42, 10], [150, -262, 236]],
];

// The screen's story: [progress at which the next image starts fading in, fade length, image].
const SCREENS: [number, number, number][] = [
  [0.56, 0.03, 1], // review
  [0.69, 0.02, 2], // passcode
  [0.775, 0.015, 3], // signing
  [0.845, 0.02, 4], // signed
];
const SCREEN_URLS = [
  "/landing/screens/01-wallet.png",
  "/landing/screens/07-review-send.png",
  "/landing/screens/12-passcode.png",
  "/landing/screens/28-signing.png",
  "/landing/screens/15-signed.png",
];

function interpolateShot(progress: number): { target: THREE.Vector3; offset: THREE.Vector3 } {
  let i = 0;
  while (i < SHOTS.length - 2 && progress > SHOTS[i + 1]![0]) i++;
  const [p0, t0, o0] = SHOTS[i]!;
  const [p1, t1, o1] = SHOTS[i + 1]!;
  const t = ease(span(progress, p0, p1));
  const target = new THREE.Vector3(...t0).lerp(new THREE.Vector3(...t1), t);
  const offset = new THREE.Vector3(...o0).lerp(new THREE.Vector3(...o1), t);
  return { target, offset };
}

export function createBench(options: BenchOptions): Bench {
  const { canvas, labels, focus, reducedMotion } = options;
  // Transparent, so the page's Obsidian Chrome sweep is the backdrop the parts sit in.
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "high-performance" });
  renderer.setClearColor(0x000000, 0);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;

  const scene = new THREE.Scene();

  const camera = new THREE.PerspectiveCamera(30, 1, 5, 4000);
  let dirty = true;
  let progress = 0;

  // The device's frame: case Z up off the mat.
  const world = new THREE.Group();
  world.rotation.x = -Math.PI / 2;
  scene.add(world);
  const toWorld = (v: THREE.Vector3) => v.clone().applyMatrix4(world.matrixWorld);

  scene.add(new THREE.HemisphereLight(0xc4ced6, 0x0a0a0a, 0.75));
  const key = new THREE.DirectionalLight(0xffffff, 2.4);
  key.position.set(-160, 420, 260);
  key.target.position.set(40, 0, -70);
  key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048);
  key.shadow.camera.left = -260;
  key.shadow.camera.right = 260;
  key.shadow.camera.top = 260;
  key.shadow.camera.bottom = -260;
  key.shadow.camera.near = 50;
  key.shadow.camera.far = 1200;
  key.shadow.radius = 6;
  key.shadow.bias = -0.0004;
  scene.add(key, key.target);
  const rim = new THREE.DirectionalLight(0x8297a8, 1.4);
  rim.position.set(220, 160, -320);
  scene.add(rim);

  // The cutting mat.
  // No mat to see: a floor that only catches the parts' soft shadows on the sweep behind them.
  const mat = new THREE.Mesh(new THREE.PlaneGeometry(1600, 1200), new THREE.ShadowMaterial({ opacity: 0.34 }));
  mat.position.set(40, 70, MAT_Z - 0.05);
  mat.receiveShadow = true;
  world.add(mat);

  // The screen: two stacked planes so one image can fade into the next; power dims both.
  const textures = SCREEN_URLS.map((url) => {
    const texture = new THREE.TextureLoader().load(url, () => (dirty = true));
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.anisotropy = renderer.capabilities.getMaxAnisotropy();
    return texture;
  });
  const screenBase = new THREE.MeshBasicMaterial({ map: textures[0]!, color: 0x000000, toneMapped: false });
  const screenTop = new THREE.MeshBasicMaterial({ map: textures[1]!, transparent: true, opacity: 0, toneMapped: false });
  const screenPlane = new THREE.PlaneGeometry(57.6, 43.2);
  const screenMeshBase = new THREE.Mesh(screenPlane, screenBase);
  const screenMeshTop = new THREE.Mesh(screenPlane, screenTop);
  screenMeshBase.position.set(75.8, 59.03, 18.62);
  screenMeshTop.position.set(75.8, 59.03, 18.64);
  const glow = new THREE.PointLight(0x9fb8cc, 0, 140, 0);
  glow.position.set(75.8, 59.03, 34);

  const placed: Placed[] = [];
  let cap4: THREE.Object3D | null = null;
  let disposed = false;

  async function build() {
    world.updateMatrixWorld(true);
    for (const spec of partSpecs()) {
      const object = await spec.build();
      if (disposed) return;
      object.updateMatrixWorld(true);
      const bounds = new THREE.Box3().setFromObject(object);
      const centre = bounds.getCenter(new THREE.Vector3());
      const pivot = new THREE.Group();
      object.position.sub(centre);
      pivot.add(object);
      const home = centre.clone();
      const benchAt = new THREE.Vector3(spec.bench.x, spec.bench.y, MAT_Z + (centre.z - bounds.min.z));
      pivot.position.copy(benchAt);
      pivot.rotation.z = spec.bench.turn;
      world.add(pivot);
      const size = bounds.getSize(new THREE.Vector3());
      const depth = spec.bench.turn ? size.x : size.y;
      // At the part's top surface, so a tall part's own height can't hide its label.
      const label = new THREE.Vector3(spec.bench.x, spec.bench.y + depth / 2 + 4, MAT_Z + size.z);
      placed.push({ spec, pivot, home, benchAt, label });
      if (spec.id === "screen") {
        // The screen planes ride with the screen module; its children keep case coordinates.
        object.add(screenMeshBase, screenMeshTop, glow);
      }
      if (spec.id === "keys") cap4 = object.getObjectByName("cap-4") ?? null;
      dirty = true;
    }
    options.onReady?.();
  }
  build().catch((error: unknown) => options.onError?.(error));

  function layout() {
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (!width || !height) return;
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    dirty = true;
  }

  function apply(time: number) {
    const p = progress;
    // Parts travel from the bench to their seats, arcing over the others.
    // Each part rises off the mat, travels to above its seat, then drops straight in, so it never passes
    // through the parts already seated.
    for (const part of placed) {
      const t = span(p, part.spec.window[0], part.spec.window[1]);
      const rise = ease(span(t, 0, 0.3));
      const travel = ease(span(t, 0.12, 0.72));
      const drop = ease(span(t, 0.7, 1));
      const top = part.home.z + part.spec.lift;
      part.pivot.position.lerpVectors(part.benchAt, part.home, travel);
      part.pivot.position.z = part.benchAt.z + (top - part.benchAt.z) * rise - part.spec.lift * drop;
      part.pivot.rotation.z = part.spec.bench.turn * (1 - travel);
    }

    // The screen powers on, then steps through the signing screens.
    const power = ease(span(p, 0.47, 0.53));
    let shown = 0;
    let next = 0;
    let fade = 0;
    for (const [start, length, image] of SCREENS) {
      if (p >= start + length) shown = image;
      else if (p >= start) {
        next = image;
        fade = (p - start) / length;
        break;
      }
    }
    screenBase.map = textures[shown]!;
    screenBase.color.setScalar(power);
    screenTop.map = textures[next || shown]!;
    screenTop.opacity = next ? fade * power : 0;
    glow.intensity = 1.6 * power;

    // Key 4 approves the transfer.
    if (cap4) cap4.position.z = 19.6 - 1.6 * Math.sin(Math.PI * span(p, 0.64, 0.67));

    // The camera, with a slow drift while the bench waits.
    const { target, offset } = interpolateShot(p);
    const aspect = camera.aspect || 1;
    // On a tall, narrow screen, keep the whole device in frame rather than its screen alone.
    if (aspect < 1 && p > 0.4) target.lerp(new THREE.Vector3(52, 42, 10), 0.55);
    const fit = aspect < 1.1 ? Math.min(2.1, 1.25 / Math.max(aspect, 0.45)) : 1;
    offset.multiplyScalar(fit);
    if (!reducedMotion && p < 0.08) {
      const a = time / 4200;
      offset.x += Math.sin(a) * 14 * (1 - p / 0.08);
      offset.y += Math.cos(a * 0.8) * 8 * (1 - p / 0.08);
    }
    // Shift the projection so the scene's centre lands where the page wants it.
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    const at = focus(p);
    camera.setViewOffset(width, height, -(at.x - 0.5) * width, -(at.y - 0.5) * height, width, height);
    camera.updateProjectionMatrix();
    const targetWorld = toWorld(target);
    camera.position.copy(toWorld(target.clone().add(offset)));
    camera.lookAt(targetWorld);
    camera.updateMatrixWorld();

    // Part labels follow their parts until the parts leave the bench.
    const labelAlpha = 1 - span(p, 0.03, 0.09);
    for (const { id, element } of labels) {
      const part = placed.find((entry) => entry.spec.id === id);
      if (!part || labelAlpha <= 0) {
        element.style.opacity = "0";
        continue;
      }
      const anchor = toWorld(part.label);
      anchor.project(camera);
      const x = ((anchor.x + 1) / 2) * canvas.clientWidth;
      const y = ((1 - anchor.y) / 2) * canvas.clientHeight;
      element.style.opacity = String(labelAlpha);
      element.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px) translate(-50%, -100%)`;
    }
  }

  const observer = new ResizeObserver(layout);
  observer.observe(canvas);
  layout();

  let frame = 0;
  let visible = true;
  const visibility = new IntersectionObserver(([entry]) => {
    visible = entry?.isIntersecting ?? true;
  });
  visibility.observe(canvas);

  const loop = (time: number) => {
    frame = requestAnimationFrame(loop);
    if (!visible) return;
    const drifting = !reducedMotion && progress < 0.08;
    if (!dirty && !drifting) return;
    apply(time);
    renderer.render(scene, camera);
    dirty = false;
  };
  frame = requestAnimationFrame(loop);

  return {
    setProgress(next: number) {
      if (next !== progress) {
        progress = next;
        dirty = true;
      }
    },
    dispose() {
      disposed = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      visibility.disconnect();
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh) {
          object.geometry.dispose();
          const materials = Array.isArray(object.material) ? object.material : [object.material];
          for (const material of materials) {
            (material as THREE.MeshStandardMaterial).map?.dispose();
            material.dispose();
          }
        }
      });
      for (const texture of textures) texture.dispose();
      renderer.dispose();
    },
  };
}
