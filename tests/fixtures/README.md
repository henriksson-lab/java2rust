# Test fixtures

`demo.jar` is built from `src/` (an annotated `org.demo.Store` that references the
deliberately-omitted `org.external.Widget`) to exercise `jar-to-symbols`:

    cd tests/fixtures
    javac -d /tmp/cls src/org/demo/*.java src/org/external/*.java
    (cd /tmp/cls && jar cf - org/demo) > demo.jar   # note: omits org/external on purpose
