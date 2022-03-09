# `Imagescripts-rs`

A collection of modules for image gallery manipulations, such as: creating animations and slideshows from images in `.zip` archives or in folder respectively; comparing image encoding commands (e.g. `cjxl` for _jpeg xl_ or `avifenc` for _avif_) (by size, metrics and time); finding images with desired bpp (bits per pixel), monochrome images, or images with specified dimmensions.

- [`Imagescripts-rs`](#imagescripts-rs)
- [Features](#features)
  - ['Finding' images (`find`)](#finding-images-find)
    - [Find images with desired bpp (`bpp`)](#find-images-with-desired-bpp-bpp)
    - [Find Monochrome images (`monochrome`)](#find-monochrome-images-monochrome)
    - [Find images by dimmensions (`resizable`)](#find-images-by-dimmensions-resizable)
    - [Find similar images (`similar`)](#find-similar-images-similar)
  - [Animation / Slideshow creation (`gen`)](#animation--slideshow-creation-gen)
    - [Slideshow from images in folder (`video`)](#slideshow-from-images-in-folder-video)
    - [Animation from `.zip`: frames + json (`zip2video`)](#animation-from-zip-frames--json-zip2video)
  - [Image encoders comparison (`cmds`)](#image-encoders-comparison-cmds)

# Features

> Each 'module' has a `--help` switch

## 'Finding' images (`find`)

### Find images with desired bpp (`bpp`)

Find images that have a bpp value less/greater than the target value. There is also a custom metric behind the `-m` switch: `bpp + px_count / 2048^2`

**Example**

```bash
ims-rs find bpp -m -l 3.5
```

### Find Monochrome images (`monochrome`)

Checks if the image is monochrome by computing Mean Squared Error (x100) from mean hue bias by converting each pixel of the thumbnailed image to hsv

**Example**

```bash
ims-rs find monochrome --nproc 16 -t 0.8 -o "./monochrome"
ims-rs find monochrome --nproc 16 -t 1000 -o "./maybe_monochrome"
```

### Find images by dimmensions (`resizable`)

Find 'resizable' images with any dimmension larger than the target size, with possible `.png` separation.  
Default target: 3508px

**Example**

```bash
ims-rs find resizable -s 4961 --p --keep-empty
```

### Find similar images (`similar`)

Find similar images using image hashes

## Animation / Slideshow creation (`gen`)

### Slideshow from images in folder (`video`)

Creates slideshow (default fps: `-r 2`). The video dimmension based on average image size.

**Example:**

```bash
ims-rs gen video -f x264 -c mkv
```

### Animation from `.zip`: frames + json (`zip2video`)

The `.js` or `.json` file will be searched in the zip file (w/ any name) or the folder with the zip file (name of zip + .js/.json)

**json structure**

- `.js` :

```json
{...,
  "frames": [
    {
      "file": "123.png",
      "delay": 200
    }, ...] }
```

- `.json` :

```json
{"..." :
  {...,
    "frames": [
      {
        "file": "123.png",
        "delay": 200
      }, ...] } }
```

**Example:**

```bash
ims-rs gen zip2video *.zip
```

## Image encoders comparison (`cmds`)

Supports output to cli/csv

**Example:**

```bash
ims-rs cmds --save --csv -c \
    "avif:--min 0 --max 10" -c \
    "cjxl:-d 1" -- ./1.png
```

**Example output:**

```bash
avifenc --min 0 --max 10
117.0KiB --> 10.0KiB      0.04bpp         0.59s         8%
cjxl -d 1
117.0KiB --> 10.8KiB      0.04bpp         1.32s         9%
```

