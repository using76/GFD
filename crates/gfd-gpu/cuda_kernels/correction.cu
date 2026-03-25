// correction.cu — velocity and pressure correction kernels for SIMPLE/PISO
//
// Each thread handles one cell.

// ---------------------------------------------------------------------------
// Velocity correction: u* = u - (dt / rho) * grad(p')
// ---------------------------------------------------------------------------

extern "C" __global__
void correct_velocity(
    double*       __restrict__ vel_x,
    double*       __restrict__ vel_y,
    double*       __restrict__ vel_z,
    const double* __restrict__ grad_p_x,
    const double* __restrict__ grad_p_y,
    const double* __restrict__ grad_p_z,
    double dt,
    double rho,
    int num_cells)
{
    int c = blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= num_cells) return;

    double factor = dt / rho;
    vel_x[c] -= factor * grad_p_x[c];
    vel_y[c] -= factor * grad_p_y[c];
    vel_z[c] -= factor * grad_p_z[c];
}

// ---------------------------------------------------------------------------
// Pressure correction: p = p + alpha_p * p'
// ---------------------------------------------------------------------------

extern "C" __global__
void correct_pressure(
    double*       __restrict__ pressure,
    const double* __restrict__ pressure_correction,
    double alpha_p,
    int num_cells)
{
    int c = blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= num_cells) return;

    pressure[c] += alpha_p * pressure_correction[c];
}
