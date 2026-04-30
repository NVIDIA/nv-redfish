// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::marker::PhantomData;

use tokio::task::JoinHandle;

use crate::QueryId;
use crate::Scraper;

/// Background watch handle.
pub struct QueryWatch<B, T> {
    pub(super) scraper: Scraper<B>,
    pub(super) query_id: QueryId,
    pub(super) task: Option<JoinHandle<()>>,
    pub(super) active: bool,
    pub(super) resource_type: PhantomData<fn() -> T>,
}

impl<B, T> Debug for QueryWatch<B, T>
where
    B: Debug,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("QueryWatch")
            .field("scraper", &self.scraper)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl<B, T> Drop for QueryWatch<B, T> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if self.active {
            self.scraper
                .inner()
                .queries
                .unregister_long_lived(self.query_id, &self.scraper.inner().events);
            let _ignored = self.scraper.inner().store.remove_query(self.query_id);
            self.active = false;
        }
    }
}
