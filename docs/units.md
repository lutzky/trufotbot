<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Units

TrufotBot does not have a dedicated "unit" field for medications. Every dose
quantity, [dose limit](#dose-limits), and [inventory](#inventory) count is
recorded as a plain number.

Tracking and converting units (`mg`, `ml`, pills, puffs, drops) in software adds
friction to everyday logging, whereas people in a household already have a
natural way of talking about each medication. Instead of enforcing a formal unit
system, TrufotBot relies on establishing a per-medication "unit" in the
medication's **description** field.

## Defining a Unit in the Description

A medication's name and description are shared across all patients in the
household, and the description is displayed right under the medication's name on
its page. Use it to write down what `1` means for that medication so everyone in
the household logs doses consistently.

### Examples

- **Pills that come in multiple strengths:** For Ibuprofen, set the description
  to `200mg, 400mg counts as 2`. Here, `1` unit is a 200mg pill; if you take a
  400mg pill, record a quantity of `2`.
- **Single-strength pills or tablets:** Set the description to `500mg pills` (or
  `1 pill = 500mg`), and log the number of pills taken (`1`, `2`, `0.5`, etc.).
- **Liquid medications:** For a children's syrup such as Nurofen Kids, set the
  description to `ml` (or `in ml, 100mg/5ml`), and log the syringe volume
  directly (e.g. `7.5`).
- **Drops, sprays, or inhalers:** Set the description to `drops` or `puffs`, and
  log the count.

## Dose Limits

Dose limits are configured per patient on each medication's edit page as a
comma-separated list of `hours:amount` pairs, using the same unit established in
the medication's description.

For example, with Ibuprofen described as `200mg, 400mg counts as 2`, a limit of
`4:2,24:6` means:

- Up to **2 units** (2 × 200mg = 400mg) every **4 hours**
- No more than **6 units** (6 × 200mg = 1200mg) every **24 hours**

Because a 400mg pill is logged as `2`, taking one 400mg pill uses the full
4-hour allowance of `2` and counts as `2` toward the 24-hour allowance of `6`.

Because the medication description is shared while dose limits are configured
per patient, different family members can have different limits for the same
medication (or copy another patient's limits with one click) while sharing the
same unit convention.

## Inventory

If you track inventory for a medication, the inventory count uses the same unit
as doses and limits: logging a dose of `2` reduces the inventory by `2`.

In the Ibuprofen example above (`200mg, 400mg counts as 2`), a pack of ten
400mg pills is `20` units of inventory.

## Logging Doses and Natural Input

Whether you record a dose from the web UI, a reminder button (**Take 2 💊**),
[`/record`](interacting.md#record), or
[natural input](interacting.md#natural-input), the quantity is always in the
medication's unit:

- In [natural input](interacting.md#natural-input), any trailing unit suffix you
  type (such as `Nurofen kids 7.5ml`) is parsed and ignored — only the number
  (`7.5`) is recorded.
- Because unit suffixes are not converted, always write the number of units
  rather than a raw milligram amount (for example, write `Ibuprofen 2` — not
  `Ibuprofen 400mg` — when taking a 400mg pill under the
  `200mg, 400mg counts as 2` convention).
