/**
 * Real constructive-solid-geometry (CSG) boolean operations using three-bvh-csg.
 * Takes two store Shapes, produces a new "stl" Shape carrying the merged vertex data.
 */
import * as THREE from 'three';
import { Brush, Evaluator, ADDITION, SUBTRACTION, INTERSECTION } from 'three-bvh-csg';
import type { Shape, BooleanOp } from '../store/useAppStore';

const degToRad = (d: number) => (d * Math.PI) / 180;

function buildGeometry(shape: Shape): THREE.BufferGeometry {
  const d = shape.dimensions ?? {};
  switch (shape.kind) {
    case 'box':
    case 'enclosure':
      return new THREE.BoxGeometry(d.width ?? 1, d.height ?? 1, d.depth ?? 1);
    case 'sphere':
      return new THREE.SphereGeometry(d.radius ?? 0.5, 32, 24);
    case 'cylinder':
      return new THREE.CylinderGeometry(d.radius ?? 0.3, d.radius ?? 0.3, d.height ?? 1, 32);
    case 'cone':
      return new THREE.ConeGeometry(d.radius ?? 0.4, d.height ?? 1, 32);
    case 'torus':
      return new THREE.TorusGeometry(d.majorRadius ?? 0.5, d.minorRadius ?? 0.15, 16, 48);
    case 'pipe': {
      // Approximate pipe as outer cylinder minus inner cylinder (no real hole — CSG handles it)
      const ro = d.outerRadius ?? 0.4;
      const h = d.height ?? 1.5;
      return new THREE.CylinderGeometry(ro, ro, h, 32);
    }
    case 'stl':
      if (shape.stlData) {
        const geo = new THREE.BufferGeometry();
        // Clone to avoid sharing buffer with display
        geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(shape.stlData.vertices), 3));
        geo.computeVertexNormals();
        return geo;
      }
      return new THREE.BoxGeometry(0.5, 0.5, 0.5);
    default:
      return new THREE.BoxGeometry(0.5, 0.5, 0.5);
  }
}

function applyTransform(brush: Brush, shape: Shape): void {
  brush.position.set(shape.position[0], shape.position[1], shape.position[2]);
  brush.rotation.set(degToRad(shape.rotation[0]), degToRad(shape.rotation[1]), degToRad(shape.rotation[2]));
  brush.updateMatrixWorld(true);
}

/** Run a CSG boolean on two shapes and return a new "stl" shape carrying the
 *  merged geometry. Returns null if the operation doesn't produce vertices. */
export function performCsgBoolean(
  op: BooleanOp,
  target: Shape,
  tool: Shape,
  newId: string,
): Shape | null {
  if (op === 'split') {
    // Split is not a boolean; skip.
    return null;
  }
  let threeOp;
  if (op === 'union') threeOp = ADDITION;
  else if (op === 'subtract') threeOp = SUBTRACTION;
  else threeOp = INTERSECTION;

  const geoA = buildGeometry(target);
  const geoB = buildGeometry(tool);

  const brushA = new Brush(geoA);
  applyTransform(brushA, target);
  brushA.prepareGeometry();

  const brushB = new Brush(geoB);
  applyTransform(brushB, tool);
  brushB.prepareGeometry();

  const evaluator = new Evaluator();
  const resultBrush = evaluator.evaluate(brushA, brushB, threeOp);

  // Extract vertex data
  const resultGeom = resultBrush.geometry as THREE.BufferGeometry;
  const posAttr = resultGeom.getAttribute('position') as THREE.BufferAttribute | undefined;
  if (!posAttr || posAttr.count === 0) {
    geoA.dispose();
    geoB.dispose();
    return null;
  }

  const nonIndexed = resultGeom.index ? resultGeom.toNonIndexed() : resultGeom;
  const nonIndexedPos = nonIndexed.getAttribute('position') as THREE.BufferAttribute;
  const nVerts = nonIndexedPos.count;
  const fc = Math.floor(nVerts / 3);
  const vertices = new Float32Array(nVerts * 3);
  for (let i = 0; i < nVerts; i++) {
    vertices[i * 3] = nonIndexedPos.getX(i);
    vertices[i * 3 + 1] = nonIndexedPos.getY(i);
    vertices[i * 3 + 2] = nonIndexedPos.getZ(i);
  }

  geoA.dispose();
  geoB.dispose();
  if (nonIndexed !== resultGeom) nonIndexed.dispose();

  return {
    id: newId,
    name: `${op}(${target.name},${tool.name})`,
    kind: 'stl',
    position: [0, 0, 0], // vertices are already in world space
    rotation: [0, 0, 0],
    dimensions: {},
    stlData: { vertices, faceCount: fc },
    group: 'boolean',
  };
}
