# Klipper GCode Preprocessor for Object Cancellation

This is a Rust port of the excellent preprocess-cancellation preprocessor by Frank Tackitt. The preprocessor
modifies GCode files to add Klipper's exclude object gcode.

The following slicers are currently supported:

* SuperSlicer
* PrusaSlicer
* Slic3r
* Cura
* IdeaMaker

## Why the rewrite

The SBCs typically running Klipper are not very beefy and when uploading GCode files that are 
100+ MB large the upload often times out during preprocessing of the files. This rewrite tries
to solve that problem and keep the convenience of having files automatically processed on the 
host without having to deal with Slicer integration.

Testing on an M1 Macbook shows a 10x improvement in processing speed, from 48s down to <5s for
a reasonably complex file of about 130MB.

```bash
time preprocess_cancellation-macos  demo/plate_1.gcode
preprocess_cancellation-macos   32.57s user 1.55s system 70% cpu 48.234 total
```

```bash
preprocess-cancellation-rs   3.85s user 0.13s system 93% cpu 4.276 total
```

## Installation and usage

### SuperSlicer, PrusaSlicer, and Slic3r

Download the provided binary for your platform, and place it in with in your slicer's folder.

In your Print Settings, under Output Options, add `preprocess_cancellation.exe;` to the
"Post-Processing Scripts". For mac or linux, you should just use `preprocess_cancellation;`

Then, all generated gcode should be automatically processed and rewritten to support cancellation.

### G-Codes for Object Cancellation

There are 3 gcodes inserted in the files automatically, and 4 more used to control the
object cancellation.

`EXCLUDE_OBJECT_DEFINE NAME=<object name> [CENTER=x,y] [POLYGON=[[x,y],...]]`

The NAME must be unique and consistent throughout the file. CENTER is the center location
for the mesh, used to show on interfaces where and object being canceled is on the bed.
POLYGON is a series of points, used to represent the bounds of the object. It can be just
a bounding box, a simplified outline, or another useful shape.

`EXCLUDE_OBJECT_START NAME=<object name>` and `EXCLUDE_OBJECT_END [NAME=<object name>]`

The beginning and end markers for the gcode for a single object. When an object is excluded,
anything between these markers is ignored.

For a full breakdown, see [the klipper G-Code Reference](https://www.klipper3d.org/G-Codes.html#excludeobject)

### Known Limitations

Cura and Ideamaker sliced files have all support material as a single non-mesh entity.
This means that when canceling an object, it's support will still print. Including
support that is inside or built onto the canceled mesh. The Slic3r family (including
PrusaSlicer and SuperSlicer) treat support as part of the individual mesh's object,
so canceling a mesh cancels it's support as well.

### How does it work

This looks for known markers inside the GCode, specific to each slicer. It uses those
to figure out the printing object's name, and track all extrusion moves within its
print movements. Those are used to calculate a minimal bounding box for each mesh.
A series of `EXCLUDE_OBJECT_DEFINE` gcodes are placed in a header, including the bounding boxes
and objects centers. Then, these markers are used to place `EXCLUDE_OBJECT_START` and
`EXCLUDE_OBJECT_END` gcodes in the file surrounding each set of extrusions for that object.
