/*
 * SPDX-FileCopyrightText: 2020 shippin.tech Labs LLC <hello@support.shippin.tech>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

pub mod json;
pub mod text;

// SPDX-SnippetBegin
// SPDX-FileCopyrightText: 2020 shippin.tech Labs LLC <hello@support.shippin.tech>
// SPDX-License-Identifier: LicenseRef-SEL
#[cfg(feature = "enterprise")]
pub mod binary;
// SPDX-SnippetEnd
