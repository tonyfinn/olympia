A native Olympia GUI using GTK

## Note on testing:

GTK only allows access to GTK widgets from one thread. Cargo by default
runs tests across many threads. This means that threads that access GTK
widgets would cause failures as they destroy GTK's internal state if run
on different threads. Since the tests are normally run in the context
of the entire olympia suite, running the entire suite single threaded
is suboptimal.

To allow for parallelism to be used for tests across the whole project, but
also for tests that actually use GTK to be written, the GTK using tests
are prefixed with `gtk_`. 


`test.sh` then runs two passes, the first a multithreaded run that skips those
tests and runs others, the second a single threaded run that runs only thoses
tests. `.tarpaulin.yaml` configures tarpaulin in a similar manner.