#!/usr/bin/env python3
"""
GFD Result Visualizer — reads VTK output and creates 2D plots.
Usage: python tools/visualize.py results/pipe_flow/result.vtk
"""
import sys
import os
import re
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.tri import Triangulation
from matplotlib.colors import Normalize
import matplotlib.cm as cm

def parse_vtk(filepath):
    """Parse ASCII VTK unstructured grid file."""
    with open(filepath, 'r') as f:
        lines = f.readlines()

    i = 0
    points = []
    cells = []
    cell_types = []
    scalars = {}
    vectors = {}
    n_points = 0
    n_cells = 0

    while i < len(lines):
        line = lines[i].strip()

        if line.startswith('POINTS'):
            n_points = int(line.split()[1])
            i += 1
            for _ in range(n_points):
                parts = lines[i].strip().split()
                points.append([float(x) for x in parts[:3]])
                i += 1
            continue

        elif line.startswith('CELLS '):
            parts = line.split()
            n_cells = int(parts[1])
            i += 1
            for _ in range(n_cells):
                parts = lines[i].strip().split()
                n_verts = int(parts[0])
                cell_nodes = [int(x) for x in parts[1:n_verts+1]]
                cells.append(cell_nodes)
                i += 1
            continue

        elif line.startswith('CELL_TYPES'):
            n = int(line.split()[1])
            i += 1
            for _ in range(n):
                cell_types.append(int(lines[i].strip()))
                i += 1
            continue

        elif line.startswith('SCALARS'):
            name = line.split()[1]
            i += 1  # skip LOOKUP_TABLE
            if 'LOOKUP_TABLE' in lines[i]:
                i += 1
            vals = []
            while len(vals) < n_cells and i < len(lines):
                for v in lines[i].strip().split():
                    try:
                        vals.append(float(v))
                    except ValueError:
                        break
                i += 1
            scalars[name] = np.array(vals[:n_cells])
            continue

        elif line.startswith('VECTORS'):
            name = line.split()[1]
            i += 1
            vals = []
            while len(vals) < n_cells and i < len(lines):
                parts = lines[i].strip().split()
                if len(parts) >= 3:
                    try:
                        vals.append([float(parts[0]), float(parts[1]), float(parts[2])])
                    except ValueError:
                        break
                i += 1
            vectors[name] = np.array(vals[:n_cells])
            continue

        i += 1

    return np.array(points), cells, scalars, vectors

def cell_centers(points, cells):
    """Compute cell centers from node coordinates."""
    centers = []
    for cell in cells:
        cx = np.mean([points[n][0] for n in cell])
        cy = np.mean([points[n][1] for n in cell])
        centers.append([cx, cy])
    return np.array(centers)

def plot_results(vtk_path, output_dir=None):
    """Generate 2D visualization from VTK file."""
    print(f"Reading {vtk_path}...")
    points, cells, scalars, vectors = parse_vtk(vtk_path)
    centers = cell_centers(points, cells)

    if output_dir is None:
        output_dir = os.path.dirname(vtk_path)

    case_name = os.path.basename(os.path.dirname(vtk_path))
    if not case_name:
        case_name = os.path.splitext(os.path.basename(vtk_path))[0]

    x = centers[:, 0]
    y = centers[:, 1]
    n_plots = len(scalars) + len(vectors)
    if n_plots == 0:
        print("No data to plot!")
        return

    fig, axes = plt.subplots(1, min(n_plots, 3), figsize=(6 * min(n_plots, 3), 5))
    if n_plots == 1:
        axes = [axes]
    fig.suptitle(f'GFD Results: {case_name}', fontsize=14, fontweight='bold')

    plot_idx = 0

    # Plot scalar fields
    for name, vals in scalars.items():
        if plot_idx >= 3:
            break
        ax = axes[plot_idx]
        sc = ax.tricontourf(x, y, vals, levels=20, cmap='coolwarm')
        plt.colorbar(sc, ax=ax, shrink=0.8)
        ax.set_title(name)
        ax.set_xlabel('x')
        ax.set_ylabel('y')
        ax.set_aspect('equal')
        plot_idx += 1

    # Plot vector fields
    for name, vals in vectors.items():
        if plot_idx >= 3:
            break
        ax = axes[plot_idx]

        # Velocity magnitude
        mag = np.sqrt(vals[:, 0]**2 + vals[:, 1]**2)
        sc = ax.tricontourf(x, y, mag, levels=20, cmap='viridis')
        plt.colorbar(sc, ax=ax, shrink=0.8, label='|V|')

        # Quiver plot (subsample for clarity)
        step = max(1, len(x) // 200)
        ax.quiver(x[::step], y[::step], vals[::step, 0], vals[::step, 1],
                  color='white', scale=None, width=0.003, alpha=0.8)

        ax.set_title(f'{name} (magnitude + arrows)')
        ax.set_xlabel('x')
        ax.set_ylabel('y')
        ax.set_aspect('equal')
        plot_idx += 1

    plt.tight_layout()
    out_path = os.path.join(output_dir, f'{case_name}_plot.png')
    fig.savefig(out_path, dpi=150, bbox_inches='tight')
    print(f"Saved: {out_path}")
    plt.close()
    return out_path

if __name__ == '__main__':
    if len(sys.argv) < 2:
        # Plot all results
        results_dir = 'results'
        for case in os.listdir(results_dir):
            vtk = os.path.join(results_dir, case, 'result.vtk')
            if os.path.exists(vtk):
                try:
                    plot_results(vtk)
                except Exception as e:
                    print(f"  Error plotting {case}: {e}")
    else:
        plot_results(sys.argv[1])
