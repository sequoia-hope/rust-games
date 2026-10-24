#!/usr/bin/env python3
"""The city grid: how much of the ground is built on and how tall it stands,
for the open world's city (docs/vision/peninsula.md, stage 6b).

  python3 tools/atlas/city.py [--name bayside] [--step 32] [--refresh]

Reads the box of <name>.ground.bin (written by ground.py; the same corridors
folder), takes the real buildings inside it from Overture Maps (release
build.OVERTURE_RELEASE, theme buildings, type building: OpenStreetMap and
others, ODbL; about half a million footprints for the bay side, 80 % with a
height), and reduces them to cells of `step` game metres over that box.
Writes <name>.city.bin beside it, which `mp_levels::ground::CityGrid` reads:

      0  b"MPCITY01"
      8  u32 width, u32 height (cells)
     16  f32 step, f32 x0, f32 z0 (game metres; cell (0, 0)'s centre, the
         north-west corner, which is the ground grid's; x east, z south)
     28  u32 length of the zlib stream
     32  zlib: three u8 a cell, row by row from the north:
           coverage  the share of the cell's ground under building
                     footprints, 0 to 255 (255 all built over)
           p50       the median height of the buildings in the cell, in
                     units of 2 real metres
           p90       their ninetieth percentile, the same units

The plan's scale shrinks the ground, not the buildings (stage 6): the
footprints are placed in game metres (real local metres from build.py's
projection times the plan's scale, x east, z south, the origin Half Moon
Bay's intersection), so coverage is the real town's density, while the
heights stay real metres, as the buildings are drawn full size. The
footprints are rasterised SUB x SUB samples a cell, each sample holding the
height of the building over it (the tallest where footprints overlap): the
coverage is the share of built samples, the heights are quantiles over the
built samples, so each building counts by the area of its footprint inside
the cell, and one tower among houses shows in the ninetieth percentile as
it does on the skyline. A cell with no building is 0 everywhere.

A building without a height takes its floors times FLOOR_M (3.2 m) when
Overture has `num_floors`; otherwise a default by its class (a house or an
unclassed building 6 m, a garage or shed 3 m, apartments 12 m, offices,
hotels and hospitals 15 m, other shops, schools and works 8 m), or by its
subtype when it has no class. Heights are capped at 510 m (255 units).

The buildings are cached as parquet in tools/atlas/cache/overture/<release>/
building-building_<lon0>_<lat0>_<lon1>_<lat1>.parquet; any cached file whose
box holds the grid's is used, else the grid's box (rounded out to 0.01
degrees) is fetched from Overture's bucket on S3 (anonymous, a few
minutes). Needs numpy, Pillow, shapely 2 and pyarrow.
"""

import glob
import math
import os
import struct
import sys
import zlib

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build  # noqa: E402  (the projection, the release, the constants)

OUT = os.path.join(build.OUT, 'corridors')
# Metres a floor, for buildings with floors and no height.
FLOOR_M = 3.2
# Heights without a height or floors (m), by class, then by subtype.
CLASS_M = {
    'house': 6, 'detached': 6, 'semidetached_house': 6, 'residential': 6, 'terrace': 6,
    'bungalow': 6, 'farm': 6, 'cabin': 4,
    'garage': 3, 'garages': 3, 'shed': 3, 'carport': 3, 'roof': 3, 'shelter': 3, 'hut': 3,
    'apartments': 12, 'dormitory': 12,
    'office': 15, 'hotel': 15, 'hospital': 15,
    'commercial': 8, 'retail': 8, 'industrial': 8, 'warehouse': 8, 'school': 8, 'church': 8,
    'university': 8, 'college': 8, 'parking': 8, 'service': 8, 'civic': 8, 'public': 8,
}
SUBTYPE_M = {'residential': 6, 'outbuilding': 3, 'commercial': 8, 'industrial': 8,
             'education': 8, 'religious': 8, 'civic': 8, 'medical': 15,
             'transportation': 8, 'entertainment': 8, 'service': 8, 'agricultural': 6,
             'military': 6}
DEFAULT_M = 6
# Height units (real metres) in the file.
UNIT_M = 2.0
# Footprints are rasterised this many samples a side in each cell.
SUB = 16


def read_box(name):
    """The ground grid's box: (w, h, step, x0, z0), game metres."""
    path = os.path.join(OUT, f'{name}.ground.bin')
    with open(path, 'rb') as f:
        head = f.read(32)
    if head[:8] != b'MPGROUND':
        raise SystemExit(f'{path}: not an MPGROUND file')
    return struct.unpack('<IIfff', head[8:28])


def plan_scale():
    import json
    with open(os.path.join(build.OUT, 'plan.json')) as f:
        return float(json.load(f)['scale'])


def buildings(lon0, lat0, lon1, lat1, refresh):
    """The Overture buildings whose boxes meet the box, from the cache or S3."""
    import pyarrow.compute as pc
    import pyarrow.dataset as ds
    import pyarrow.fs as pafs
    import pyarrow.parquet as pq
    cache = os.path.join(build.CACHE, 'overture', build.OVERTURE_RELEASE)
    if not refresh:
        for path in sorted(glob.glob(os.path.join(cache, 'building-building_*.parquet'))):
            b = [float(v) for v in os.path.basename(path)[len('building-building_'):-8].split('_')]
            if b[0] <= lon0 and b[1] <= lat0 and b[2] >= lon1 and b[3] >= lat1:
                print(f'buildings: {os.path.relpath(path, build.ROOT)}', file=sys.stderr)
                return pq.read_table(path)
    b = (math.floor(lon0 * 100) / 100, math.floor(lat0 * 100) / 100,
         math.ceil(lon1 * 100) / 100, math.ceil(lat1 * 100) / 100)
    path = os.path.join(cache, 'building-building_' + '_'.join(f'{v:.2f}' for v in b) + '.parquet')
    print(f'buildings: fetching {b} from Overture {build.OVERTURE_RELEASE}', file=sys.stderr)
    s3 = pafs.S3FileSystem(anonymous=True, region='us-west-2')
    d = ds.dataset(build.OVERTURE + 'theme=buildings/type=building/', filesystem=s3, format='parquet')
    f = ((pc.field('bbox', 'xmin') < b[2]) & (pc.field('bbox', 'xmax') > b[0])
         & (pc.field('bbox', 'ymin') < b[3]) & (pc.field('bbox', 'ymax') > b[1]))
    cols = ['id', 'bbox', 'height', 'num_floors', 'class', 'subtype', 'geometry']
    t = d.to_table(filter=f, columns=cols)
    os.makedirs(cache, exist_ok=True)
    pq.write_table(t, path + '.part')
    os.replace(path + '.part', path)
    return t


def heights_of(t):
    """Each building's height (real m): its own, its floors', or a default."""
    h = t['height'].to_numpy(zero_copy_only=False).astype(np.float64)
    floors = t['num_floors'].to_numpy(zero_copy_only=False).astype(np.float64)
    cls = t['class'].to_pylist()
    sub = t['subtype'].to_pylist()
    guess = np.array([CLASS_M[c] if c in CLASS_M else SUBTYPE_M.get(s, DEFAULT_M)
                      for c, s in zip(cls, sub)], dtype=np.float64)
    out = np.where(np.isnan(h) | (h <= 0), floors * FLOOR_M, h)
    out = np.where(np.isnan(out) | (out <= 0), guess, out)
    print(f'buildings: {len(h)}; {np.mean(~np.isnan(h)):.0%} with a height, '
          f'{np.mean(np.isnan(h) & ~np.isnan(floors)):.1%} from floors, the rest by class',
          file=sys.stderr)
    return np.minimum(out, 255 * UNIT_M)


def main():
    import shapely
    args = sys.argv[1:]
    name = args[args.index('--name') + 1] if '--name' in args else 'bayside'
    step = float(args[args.index('--step') + 1]) if '--step' in args else 32.0
    refresh = '--refresh' in args
    scale = plan_scale()
    gw, gh, gstep, x0, z0 = read_box(name)
    w = int(math.ceil((gw - 1) * gstep / step)) + 1
    h = int(math.ceil((gh - 1) * gstep / step)) + 1
    # The cells' outer edges, game metres.
    ex0, ez0 = x0 - step / 2, z0 - step / 2
    ex1, ez1 = ex0 + w * step, ez0 + h * step
    lon0, lat1 = build.to_lonlat(ex0 / scale, ez0 / scale)
    lon1, lat0 = build.to_lonlat(ex1 / scale, ez1 / scale)
    t = buildings(lon0, lat0, lon1, lat1, refresh)
    hs = heights_of(t)

    # Footprints in game metres.
    geoms = shapely.from_wkb(t['geometry'].to_numpy(zero_copy_only=False))
    k_lon, k_lat = build.K_LON * scale, build.K_LAT * scale
    o_lat, o_lon = build.ORIGIN
    geoms = shapely.transform(
        geoms, lambda c: np.c_[(c[:, 0] - o_lon) * k_lon, -(c[:, 1] - o_lat) * k_lat])
    keep = shapely.intersects(geoms, shapely.box(ex0, ez0, ex1, ez1))
    geoms, hs = geoms[keep], hs[keep]
    print(f'buildings: {len(geoms)} in the box', file=sys.stderr)

    # The footprints rasterised, SUB samples a side in each cell, each
    # sample holding its building's height in file units (0: open ground);
    # the tallest drawn last, so it wins where footprints overlap.
    units = np.clip(np.round(hs / UNIT_M), 1, 255).astype(np.uint8)
    px = SUB / step
    img = Image.new('L', (w * SUB, h * SUB), 0)
    d = ImageDraw.Draw(img)
    to_px = lambda cs: [((x - ex0) * px, (z - ez0) * px) for x, z in cs]  # noqa: E731
    for k in np.argsort(units, kind='stable'):
        g = geoms[k]
        for p in getattr(g, 'geoms', [g]):
            if p.geom_type != 'Polygon' or p.is_empty:
                continue
            d.polygon(to_px(p.exterior.coords), fill=int(units[k]))
            for hole in p.interiors:
                d.polygon(to_px(hole.coords), fill=0)
    del d
    # Each cell's samples, sorted: the open ground's zeros first, then the
    # built samples' heights, so a footprint counts by its area in the cell.
    a = np.asarray(img, dtype=np.uint8).reshape(h, SUB, w, SUB).transpose(0, 2, 1, 3)
    a = np.sort(a.reshape(h * w, SUB * SUB), axis=1)
    n = SUB * SUB
    built = np.count_nonzero(a, axis=1)
    coverage = np.round(built * 255.0 / n).astype(np.uint8)
    rows = np.arange(h * w)

    def quantile(q):
        i = n - built + np.maximum(np.ceil(q * built).astype(np.int64) - 1, 0)
        return np.where(built > 0, a[rows, np.minimum(i, n - 1)], 0).astype(np.uint8)

    p50, p90 = quantile(0.5), quantile(0.9)
    grid = np.stack([coverage, p50, p90], axis=1)

    z = zlib.compress(grid.tobytes(), 9)
    head = b'MPCITY01' + struct.pack('<IIfffI', w, h, step, x0, z0, len(z))
    path = os.path.join(OUT, f'{name}.city.bin')
    with open(path, 'wb') as f:
        f.write(head)
        f.write(z)
    on = built > 0
    print(f'{path}: {w} x {h} cells of {step:g} m from ({x0:g}, {z0:g}); '
          f'{on.mean():.0%} of cells built on, mean coverage there {coverage[on].mean() / 255:.0%}, '
          f'tallest p90 {p90.max() * UNIT_M:.0f} m; {len(z) / 1024:.0f} kB', file=sys.stderr)


if __name__ == '__main__':
    main()
