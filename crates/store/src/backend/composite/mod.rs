/*
 * SPDX-FileCopyrightText: 2020 shippin.tech Labs LLC <hello@support.shippin.tech>
 *
 * SPDX-License-Identifier: LicenseRef-SEL
 *
 * This file is subject to the shippin.tech Enterprise License Agreement (SEL) and
 * is NOT open source software.
 *
 */

#[cfg(any(feature = "postgres", feature = "mysql"))]
pub mod read_replica;
pub mod sharded_blob;
pub mod sharded_lookup;
