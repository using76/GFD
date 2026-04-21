//! Regular UV-grid tessellation helper.

use crate::{TessellError, TessellResult, TriMesh};

/// Sample `(u_steps+1) × (v_steps+1)` points across the parameter rectangle
/// `[u0,u1] × [v0,v1]` and emit two triangles per cell.
pub fn uv_grid<F>(u_steps: usize, v_steps: usize, u0: f64, u1: f64, v0: f64, v1: f64, mut sample: F)
    -> TessellResult<TriMesh>
where F: FnMut(f64, f64) -> TessellResult<([f32; 3], [f32; 3])>
{
    if u_steps == 0 || v_steps == 0 {
        return Err(TessellError::Geom("zero steps".into()));
    }
    let nu = u_steps + 1;
    let nv = v_steps + 1;
    let mut mesh = TriMesh {
        positions: Vec::with_capacity(nu * nv),
        normals:   Vec::with_capacity(nu * nv),
        indices:   Vec::with_capacity(u_steps * v_steps * 6),
    };
    let du = (u1 - u0) / u_steps as f64;
    let dv = (v1 - v0) / v_steps as f64;
    for j in 0..nv {
        let v = v0 + dv * j as f64;
        for i in 0..nu {
            let u = u0 + du * i as f64;
            let (p, n) = sample(u, v)?;
            mesh.positions.push(p);
            mesh.normals.push(n);
        }
    }
    for j in 0..v_steps {
        for i in 0..u_steps {
            let a = (j * nu + i) as u32;
            let b = a + 1;
            let c = a + nu as u32;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_match() {
        let m = uv_grid(2, 2, 0.0, 1.0, 0.0, 1.0, |_u, _v| {
            Ok(([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]))
        }).unwrap();
        assert_eq!(m.positions.len(), 9);
        assert_eq!(m.indices.len(), 2 * 2 * 6);
    }
}
