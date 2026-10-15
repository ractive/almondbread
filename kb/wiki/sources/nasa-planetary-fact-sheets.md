---
title: "NASA NSSDCA planetary fact sheets (Earth, Moon, Jupiter, Sun, Neptune)"
type: source
authors: ["David R. Williams", "NASA Goddard Space Flight Center (NSSDCA)"]
year: 2024
raw: "web"
status: digested
tags: [cosmic-scale, overlay, reference-values]
---

# NASA NSSDCA planetary fact sheets

**URL:** <https://nssdc.gsfc.nasa.gov/planetary/factsheet/> (one page per body, read
2026-10-15). Maintained by D. R. Williams ("DRW") at NASA Goddard. Not saved under `raw/`.

## What it is

The standard NASA table of bulk and orbital parameters for the Sun, planets and the
Moon. Each page lists a "Last Updated" date. Values are given in km (or 10^6 km for
orbits); we convert to metres.

## Values used by the cosmic size scale

| Object | Quantity | Figure as quoted | SI | Page (last updated) |
| --- | --- | --- | --- | --- |
| Earth | Volumetric mean radius | `Volumetric mean radius (km) 6371.000` | 6.371e6 m | [earthfact](https://nssdc.gsfc.nasa.gov/planetary/factsheet/earthfact.html) (15 Nov 2024) |
| Earth | Equatorial radius | `Equatorial radius (km) 6378.137` | 6.378137e6 m | same |
| Moon | Volumetric mean radius | `Volumetric mean radius (km) 1737.4` | 1.7374e6 m | [moonfact](https://nssdc.gsfc.nasa.gov/planetary/factsheet/moonfact.html) (11 Jan 2024) |
| Moon | Semimajor axis (Earth–Moon distance) | `Semimajor axis (10^6 km) 0.3844` | 3.844e8 m | same |
| Jupiter | Volumetric mean radius | `Volumetric mean radius (km) 69,911` | 6.9911e7 m | [jupiterfact](https://nssdc.gsfc.nasa.gov/planetary/factsheet/jupiterfact.html) (2 Oct 2024) |
| Jupiter | Equatorial radius (1 bar) | `71,492` km | 7.1492e7 m | same |
| Sun | Volumetric mean radius | `Volumetric mean radius (km) 695,700` | 6.957e8 m | [sunfact](https://nssdc.gsfc.nasa.gov/planetary/factsheet/sunfact.html) (9 May 2024) |
| Neptune | Semimajor axis | `Semimajor axis (10^6 km) 4,514.953` | 4.514953e12 m (≈ 30.2 au) | [neptunefact](https://nssdc.gsfc.nasa.gov/planetary/factsheet/neptunefact.html) (3 Oct 2024) |

The Sun's 695 700 km equals the IAU 2015 nominal solar radius (IAU 2015 Resolution B3,
see [[sources/iau-astronomical-units]]).

## Reliability

High. NASA's reference tables, kept current and dated per page. Radii are mean values of
oblate bodies: quote "mean radius" in the overlay, or equatorial where that is meant.
The text conversion of the pages drops superscripts (`106 km` means 10^6 km).
