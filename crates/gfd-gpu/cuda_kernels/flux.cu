// flux.cu — compute face mass flux from cell velocity and face geometry
//
// Each thread handles one face.
//
// F_f = dot(u_f, S_f)
//     = u_f.x * Sf_x + u_f.y * Sf_y + u_f.z * Sf_z
//
// where u_f is the interpolated face velocity (simple linear average for now)
// and S_f = area * outward normal.

extern "C" __global__
void compute_face_flux(
    const double* __restrict__ vel_x,      // cell velocity x  [num_cells]
    const double* __restrict__ vel_y,      // cell velocity y  [num_cells]
    const double* __restrict__ vel_z,      // cell velocity z  [num_cells]
    const double* __restrict__ face_nx,    // face normal x    [num_faces]
    const double* __restrict__ face_ny,    // face normal y    [num_faces]
    const double* __restrict__ face_nz,    // face normal z    [num_faces]
    const double* __restrict__ face_area,  // face area        [num_faces]
    const int*    __restrict__ owner,      // owning cell idx  [num_faces]
    const int*    __restrict__ neighbor,   // neighbor cell idx[num_faces]
    double*       __restrict__ flux,       // output flux      [num_faces]
    int num_faces)
{
    int f = blockIdx.x * blockDim.x + threadIdx.x;
    if (f >= num_faces) return;

    int o = owner[f];
    int n = neighbor[f];

    // Simple linear interpolation (weight = 0.5).
    double uf_x = 0.5 * (vel_x[o] + vel_x[n]);
    double uf_y = 0.5 * (vel_y[o] + vel_y[n]);
    double uf_z = 0.5 * (vel_z[o] + vel_z[n]);

    double Sf_x = face_area[f] * face_nx[f];
    double Sf_y = face_area[f] * face_ny[f];
    double Sf_z = face_area[f] * face_nz[f];

    flux[f] = uf_x * Sf_x + uf_y * Sf_y + uf_z * Sf_z;
}
