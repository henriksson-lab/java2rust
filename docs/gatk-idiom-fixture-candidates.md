# GATK Idiom Fixture Candidates

Corpus root:
`/home/mahogny/github/claude/java2rust/testdata/gatk/src/main/java`

Sweep inputs:
`/tmp/new_java2rust_gatk_sweep/status.tsv` and
`/tmp/new_java2rust_gatk_sweep/stdout/*.rs`

These are small GATK-derived patterns where the generated Rust is accepted but
non-idiomatic, or where parser trivia/frontend gaps currently leak into output.
Each snippet is minimized from a real source reference.

## 1. Array Literal Passed to Method

Source:
`org/broadinstitute/hellbender/utils/tsv/TableWriter.java:176`

Generated signal:
`stdout/01568.rs:32` emits `/* Java array creation: String[] */ Vec::new()`,
dropping the element expression.

Reduced Java:

```java
void writeComment(String comment) {
    writer.writeNext(new String[]{PREFIX + comment}, false);
}
```

Expected idiomatic Rust shape:

```rust
writer.write_next(vec![format!("{}{}", PREFIX, comment)], false);
```

Fixture target: `examples/java`. This is valid parsed Java and should drive
array literal lowering, not a parser gap.

## 2. Sized Array Used as `toArray` Sink

Source:
`org/broadinstitute/hellbender/utils/tsv/TableWriter.java:261`

Generated signal:
`stdout/01568.rs:72` emits
`to_array(/* Java array creation: String[self.columns.column_count()] */ Vec::new())`.

Reduced Java:

```java
void writeHeader() {
    writer.writeNext(columns.names().toArray(new String[columns.columnCount()]), false);
}
```

Expected idiomatic Rust shape:

```rust
writer.write_next(columns.names().to_vec(), false);
```

Fixture target: `examples/java`. The fixture should check that computed-size
array creation used only as a collection sink does not become an empty `Vec`.

## 3. Method Reference as Function Argument

Source:
`org/broadinstitute/hellbender/utils/tsv/TableWriter.java:201`

Generated signal:
`stdout/01568.rs:46` emits
`compile_error!("unsupported Java expression: method reference")`.

Reduced Java:

```java
void writeRecord(R record) {
    DataLine line = new DataLine(lineNumber + 1, columns, IllegalArgumentException::new);
}
```

Expected idiomatic Rust shape:

```rust
let line = DataLine::new(line_number + 1, columns, |message| {
    IllegalArgumentException::new(message)
});
```

Fixture target: `examples/java`. The parser sees this expression; the idiom
work is method-reference lowering to a closure or function item.

## 4. Stream/RDD Lambda and Method References

Source:
`org/broadinstitute/hellbender/tools/spark/pathseq/PSScorer.java:65`,
`:68`, and `:71`

Generated signal:
`stdout/00517.rs:22-25` emits method-reference and lambda
`compile_error!` calls inside chained calls.

Reduced Java:

```java
JavaRDD<Hit> hits = readHits.map(Tuple2::_2);
JavaPairRDD<Integer, Score> scores =
        hits.mapPartitionsToPair(iter -> computeScores(iter, db.value(), divide));
Map<Integer, Score> map =
        new HashMap<>(scores.reduceByKey(Score::add).collectAsMap());
```

Expected idiomatic Rust shape:

```rust
let hits = read_hits.map(|tuple| tuple._2);
let scores = hits.map_partitions_to_pair(|iter| compute_scores(iter, db.value(), divide));
let map = HashMap::from(scores.reduce_by_key(Score::add).collect_as_map());
```

Fixture target: `examples/java`. This is a compact real-world callback chain
that can cover both receiver method references and lambda captures.

## 5. Class Literal in Static Logger Initialization

Source:
`org/broadinstitute/hellbender/utils/nio/NioFileCopierWithProgressMeter.java:35`

Generated signal:
`stdout/01422.rs:50` emits
`log_manager.get_logger(compile_error!("unsupported Java expression: class literal"))`.

Reduced Java:

```java
class Copier {
    private static final Logger logger = LogManager.getLogger(Copier.class);
}
```

Expected idiomatic Rust shape:

```rust
static LOGGER: Logger = LogManager::get_logger("Copier");
```

or, if the runtime keeps Java class tokens:

```rust
static LOGGER: Logger = LogManager::get_logger(class_token::<Copier>());
```

Fixture target: `examples/java`. Class literals are valid expressions and are
common in logging/serialization APIs.

## 6. `instanceof` Guard Followed by Cast

Source:
`org/broadinstitute/hellbender/tools/spark/sv/InsertSizeDistribution.java:99-103`

Generated signal:
`stdout/00556.rs:76-80` emits
`compile_error!("unsupported Java instanceof shape: actual=Concrete, target=Concrete")`
and then treats `obj` as if it had the concrete methods.

Reduced Java:

```java
public boolean equals(Object obj) {
    if (!(obj instanceof InsertSizeDistribution)) {
        return false;
    }
    return ((InsertSizeDistribution) obj).dist().equals(dist());
}
```

Expected idiomatic Rust shape:

```rust
if let Some(other) = obj.downcast_ref::<InsertSizeDistribution>() {
    other.dist().equals(self.dist())
} else {
    false
}
```

Fixture target: `examples/java`. This should become type-test plus narrowed
binding lowering rather than a generic expression fallback.

## 7. Constructor Delegates to Super Then Assigns Fields

Source:
`org/broadinstitute/hellbender/tools/copynumber/gcnv/GermlineCNVIntervalVariantComposer.java:73-79`

Generated signal:
`stdout/00314.rs:10-11` emits
`compile_error!("unsupported Java constructor: super(...) requires inheritance lowering")`.

Reduced Java:

```java
class Child extends Parent {
    private final State state;

    Child(Writer writer, String sampleName, State state) {
        super(writer, sampleName);
        this.state = Utils.nonNull(state);
    }
}
```

Expected idiomatic Rust shape:

```rust
Child {
    parent: Parent::new(writer, sample_name),
    state: Utils::non_null(state),
}
```

Fixture target: `examples/java`. The syntax is already parsed; the desired
fixture documents the object-representation shape for superclass state.

## 8. Comments Inside Executable Blocks

Source:
`org/broadinstitute/hellbender/tools/filediagnostics/CRAMAnalyzer.java:183`

Generated signal:
`stdout/00400.rs:180` emits
`compile_error!("unsupported Java statement: unsupported statement node: line_comment")`.

Reduced Java:

```java
for (Map.Entry<Integer, EncodingDescriptor> entry : map.entrySet()) {
    Integer contentID = entry.getKey(); // is this content ID?
    emit(contentID);
}
```

Expected idiomatic Rust shape:

```rust
for entry in map.entry_set() {
    let content_id = entry.get_key();
    emit(content_id);
}
```

Fixture target: `examples/parser-gaps`. Comments should be skipped as trivia in
statement lists before lowering sees the block.

## 9. Try-With-Resources Around Copy Loop

Source:
`org/broadinstitute/hellbender/utils/nio/NioFileCopierWithProgressMeter.java:402-440`

Generated signal:
`stdout/01422.rs:87` emits
`compile_error!("unsupported Java statement: try/catch/finally")`.

Reduced Java:

```java
void copy() {
    try (InputStream in = Files.newInputStream(source);
         OutputStream out = Files.newOutputStream(dest)) {
        int n = in.read(buffer);
        out.write(buffer, 0, n);
    } catch (IOException ex) {
        throw new UserException("Could not copy file", ex);
    }
}
```

Expected idiomatic Rust shape:

```rust
fn copy(&mut self) -> Result<(), JavaException> {
    let mut input = Files::new_input_stream(self.source())?;
    let mut output = Files::new_output_stream(self.dest())?;
    let n = input.read(&mut self.buffer)?;
    output.write(&self.buffer, 0, n)?;
    Ok(())
}
```

Fixture target: `examples/parser-gaps` first if the frontend cannot represent
resource specs distinctly; otherwise promote a smaller try/catch fixture to
`examples/java` for exception-result lowering.
