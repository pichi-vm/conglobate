                // The publication lock excludes cooperating creators through
                // resume and pinning. An unopened target with no active table
                // here is an abandoned pre-resume activation, not a consumer.
                // Never race an already-deferred target or remove an open one.
                const DEFERRED_REMOVE: u32 = 1 << 17;
                if status.flags & (Status::ACTIVE_TABLE | DEFERRED_REMOVE) == 0
                    && status.open_count == 0
                {
                    self.control.remove(id, false)?;
                    return Err(io::Error::from_raw_os_error(2));
                }
