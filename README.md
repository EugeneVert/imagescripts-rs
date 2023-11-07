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

Moves images that have a bpp value less/greater than the target value.

**Example**

```bash
ims-rs find bpp -l 3.5
```

### Find Monochrome images (`monochrome`)

Moves image if it's monochrome by computing Mean Squared Error (x100) from mean hue bias by converting each pixel of the thumbnailed image to hsv

**Example**

```bash
ims-rs find monochrome --nproc 16 -t 0.8 -o "./monochrome"
ims-rs find monochrome --nproc 16 -t 1000 -o "./maybe_monochrome"
```

### Find images by dimmensions (`resizable`)

Moves 'resizable' images with any dimmension larger than the target size.  
Default target: 3508px

**Example**

```bash
ims-rs find resizable -s 4961 --keep-empty
```

### Find similar images (`similar`)

Moves similar images using image hashes

## Animation / Slideshow creation (`gen`)

### Slideshow from images in folder (`video`)

Creates slideshow. The video dimmension based on average image size.

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

Utility for codecs/parameters comarison

**Example:**

```bash
ims-rs cmds --csv --save -c "avif_q(4,14)" "cjxl_d(0.7)" "cjxl_l(7)"
```

**Example output:**

```bash
./test1.png
  26.9KiB --> 19.4KiB    72% *    0.11s avifenc ...
  19.4KiB --> 45.2KiB   233%      0.03s cjxl -d 0.7 -j 0
  19.4KiB --> 12.9KiB    66% *    0.03s cjxl -d 0 -j 0 -e 7

./test2.png
 254.4KiB --> 91.5KiB    35% *    4.54s avifenc ...
  91.5KiB --> 304.9KiB  333%      0.22s cjxl -d 0.7 -j 0
  91.5KiB --> 298.7KiB  326%      0.55s cjxl -d 0 -j 0 -e 7

./test3.png
  21.3KiB --> 6.1KiB     28% *    0.10s avifenc ...
   6.1KiB --> 13.2KiB   215%      0.03s cjxl -d 0.7 -j 0
   6.1KiB --> 11.2KiB   182%      0.04s cjxl -d 0 -j 0 -e 7

stats: 
count    cmd
2        avifenc ...
1        cjxl -d 0 -j 0 -e 7
```