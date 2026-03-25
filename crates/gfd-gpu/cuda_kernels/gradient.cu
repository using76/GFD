// gradient.cu — Green-Gauss cell gradient of a scalar field
//
// For each face the contribution to the owning and neighbouring cell gradients
// is accumulated via atomicAdd:
//
//   grad(phi)_C += phi_f * S_f / V_C
//
// Boundary faces (neighbor == -1) only contribute to the owner cell.

extern "C" __global__
void green_gauss_gradient(
    const double* __restrict__ phi,        // cell-centred scalar [num_cells]
    const double* __restrict__ face_nx,    // face normal x       [num_faces]
    const double* __restrict__ face_ny,    // face normal y       [num_faces]
    const double* __restrict__ face_nz,    // face normal z       [num_faces]
    const double* __restrict__ face_area,  // face area           [num_faces]
    const int*    __restrict__ owner,      // owner cell index    [num_faces]
    const int*    __restrict__ neighbor,   // neighbor cell index [num_faces] (-1 = boundary)
    const double* __restrict__ cell_vol,   // cell volume         [num_cells]
    double*       __restrict__ grad_x,     // output grad x       [num_cells]
    double*       __restrict__ grad_y,     // output grad y       [num_cells]
    double*       __restrict__ grad_z,     // output grad z       [num_cells]
    int num_faces)
{
    int f = blockIdx.x * blockDim.x + threadIdx.x;
    if (f >= num_faces) return;

    int o = owner[f];
    int n = neighbor[f];

    // Face value: linear interpolation for internal faces, owner value for
    // boundary faces (zero-gradient assumption).
    double phi_f;
    if (n >= 0) {
        phi_f = 0.5 * (phi[o] + phi[n]);
    } else {
        phi_f = phi[o];
    }

    double Sf_x = face_area[f] * face_nx[f];
    double Sf_y = face_area[f] * face_ny[f];
    double Sf_z = face_area[f] * face_nz[f];

    // Owner contribution (positive normal direction).
    atomicAdd(&grad_x[o],  phi_f * Sf_x / cell_vol[o]);
    atomicAdd(&grad_y[o],  phi_f * Sf_y / cell_vol[o]);
    atomicAdd(&grad_z[o],  phi_f * Sf_z / cell_vol[o]);

    // Neighbour contribution (negative normal direction).
    if (n >= 0) {
        atomicAdd(&grad_x[n], -phi_f * Sf_x / cell_vol[n]);
        atomicAdd(&grad_y[n], -phi_f * Sf_y / cell_vol[n]);
        atomicAdd(&grad_z[n], -phi_f * Sf_z / cell_vol[n]);
    }
}
