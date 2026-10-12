mod common;

use common::{GeneratedCrateModule, cargo_check_generated_modules, cargo_test_generated_modules};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn batch_source_hierarchy_collection_local_uses_global_carrier_shape() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "HeaderLine.java".to_string(),
            r#"public class HeaderLine { String key; }"#.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "FormatLine.java".to_string(),
            r#"public final class FormatLine extends HeaderLine {}"#.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "InfoLine.java".to_string(),
            r#"public final class InfoLine extends HeaderLine {}"#.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "HeaderUse.java".to_string(),
            r#"
import java.util.LinkedHashSet;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

public class HeaderUse {
    Set<HeaderLine> scan() {
        Set<HeaderLine> lines = new LinkedHashSet<HeaderLine>();
        lines.add(new FormatLine());
        lines.add(new InfoLine());
        return lines;
    }
}
"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let rust = translations
        .iter()
        .find(|translation| translation.path() == "HeaderUse.java")
        .expect("translated collection user")
        .rust();

    assert!(
        rust.contains("let mut lines: JavaSemanticHashSet<HeaderLineValue>")
            && rust.contains("Into::<HeaderLineValue>::into")
            && !rust.contains("Vec<std::rc::Rc<HeaderLine>>"),
        "{rust}"
    );
    let carrier_rust = translations
        .iter()
        .find(|translation| translation.path() == "HeaderLine.java")
        .expect("translated hierarchy carrier")
        .rust();
    assert!(
        carrier_rust.contains("impl JavaMapKey for HeaderLineValue")
            && carrier_rust.contains("self.java_identity_eq(other)"),
        "{carrier_rust}"
    );
}

#[test]
fn batch_source_hierarchy_semantic_set_uses_java_equals_contract() {
    let sources = vec![new_java2rust::BatchSource::new(
        "SemanticSet.java".to_string(),
        r#"
import java.util.LinkedHashSet;
import java.util.Set;

class SemanticBase {
    public boolean equals(Object other) {
        return other instanceof SemanticBase;
    }
}
final class SemanticLeft extends SemanticBase {}
final class SemanticRight extends SemanticBase {}
class SemanticSet {
    SemanticBase missing() {
        return null;
    }

    int distinctCount() {
        Set<SemanticBase> values = new LinkedHashSet<SemanticBase>();
        values.add(new SemanticLeft());
        values.add(new SemanticRight());
        return values.size();
    }

    int nullableDistinctCount() {
        Set<SemanticBase> nullableValues = new LinkedHashSet<SemanticBase>();
        nullableValues.add(missing());
        nullableValues.add(missing());
        nullableValues.add(new SemanticLeft());
        return nullableValues.size();
    }

    int removeAllCount() {
        Set<SemanticBase> values = new LinkedHashSet<SemanticBase>();
        List<SemanticBase> removed = new ArrayList<SemanticBase>();
        values.add(new SemanticLeft());
        removed.add(new SemanticRight());
        values.removeAll(removed);
        return values.size();
    }
}
"#
        .to_string(),
    )];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let rust = translations[0].rust();
    assert!(
        rust.contains("JavaSemanticHashSet<SemanticBaseValue>")
            && rust.contains("JavaSemanticHashSet<Option<SemanticBaseValue>>")
            && rust.contains("impl JavaMapKey for SemanticBaseValue")
            && rust.contains("self.equals(match other")
            && rust.contains("java_iter_contains_by_map_key")
            && !rust.contains("self.java_identity_eq(other)"),
        "{rust}"
    );
    cargo_test_generated_modules(
        "batch_source_hierarchy_semantic_set_java_equals",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: rust.to_string(),
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn source_equals_controls_set_uniqueness() {
        assert_eq!(SemanticSet {}.distinct_count(), 1);
        assert_eq!(SemanticSet {}.nullable_distinct_count(), 2);
        assert_eq!(SemanticSet {}.remove_all_count(), 0);
    }
}
"#,
        }],
    );
}

#[test]
fn batch_field_receiver_shadows_same_normalized_source_type_name() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "Parser.java".to_string(),
        r#"
class RecordSource {
    String next() { return "record"; }
}

class Parser {
    RecordSource parser;

    String read() {
        return parser.next();
    }
}
"#
        .to_string(),
    )];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let rust = translations[0].rust();

    assert!(
        rust.contains(".next()")
            && !rust.contains("Parser::next")
            && !rust.contains("unresolved Java static call"),
        "field receiver must retain instance-call identity even when `parser` collides with `Parser`\n{rust}"
    );
}

#[test]
fn batch_inherited_wildcard_method_keeps_source_hierarchy_carrier_argument() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "CarrierObservation.java".to_string(),
            "package carrier; public class CarrierObservation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "ConcreteCarrierObservation.java".to_string(),
            "package carrier; public class ConcreteCarrierObservation extends CarrierObservation {}"
                .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierModel.java".to_string(),
            r#"package carrier;
public class CarrierModel<O extends CarrierObservation> {
    int count = 7;
    public void touch() { count = count; }
    public int size() { return count; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierSpecialModel.java".to_string(),
            r#"package carrier;
public class CarrierSpecialModel<O extends CarrierObservation> extends CarrierModel<O> {
    public int size() { return count; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierDrawer.java".to_string(),
            r#"package carrier;
public class CarrierDrawer<H extends CarrierModel<?>> {
    public int write(H model) { return model.size(); }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "GenericCarrierDrawer.java".to_string(),
            r#"package carrier;
public class GenericCarrierDrawer extends CarrierDrawer<CarrierModel<?>> {}"#.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierUse.java".to_string(),
            r#"package carrier;
public class CarrierUse {
    public static int run(CarrierModel<ConcreteCarrierObservation> model) {
        return new GenericCarrierDrawer().write(model);
    }
}"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let use_module = translations
        .iter()
        .find(|translation| translation.path() == "CarrierUse.java")
        .expect("use translation")
        .rust();
    assert!(
        use_module.contains("model: CarrierModelValue<")
            && use_module.contains(".write(model)")
            && !use_module.contains("match model { CarrierModelValue::"),
        "{use_module}"
    );
}

#[test]
fn batch_nullable_enhanced_for_collection_borrows_before_owned_recovery() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "Observation.java".to_string(),
            "package repro; public abstract class Observation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Holder.java".to_string(),
            r#"package repro;
import java.util.List;
public class Holder<T extends Observation> {
    private List<? extends T> values;
    public Holder(List<? extends T> values) { this.values = values; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Learner.java".to_string(),
            r#"package repro;
import java.util.List;
public class Learner {
    public <O extends Observation> Holder<O> consume(List<? extends O> values) {
        return new Holder<O>(values);
    }
    public <O extends Observation> void run(List<? extends List<? extends O>> groups) {
        for (List<? extends O> values : groups) consume(values);
    }
    public <O extends Observation> void allowNullGroup(List<List<O>> groups) {
        groups.set(0, null);
        run(groups);
    }
}"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let learner = translations
        .iter()
        .find(|translation| translation.path() == "Learner.java")
        .expect("learner translation")
        .rust();
    assert!(
        learner.contains("values.as_ref().ok_or_else")
            || learner.contains("values.as_ref().expect"),
        "borrowed nullable loop elements must borrow before unwrapping: {learner}"
    );
    assert!(
        !learner.contains("values.ok_or_else") && !learner.contains("values.expect"),
        "borrowed nullable loop elements must not be moved: {learner}"
    );
}

#[test]
fn batch_source_call_uses_owned_all_handle_carrier_parameter_abi() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "Model.java".to_string(),
            r#"package model;
public class Model<T> {
    int state;
    public void setState(int state) { this.state = state; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "SpecializedModel.java".to_string(),
            r#"package model;
public class SpecializedModel<T> extends Model<T> {}"#
                .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Trainer.java".to_string(),
            r#"package model;
public class Trainer<T> {
    private int state;
    private void alias(Model<T> first) {
        Model<T> second = first;
        second.setState(2);
    }
    private void aliasSpecial(SpecializedModel<T> first) {
        SpecializedModel<T> second = first;
        second.setState(3);
    }
    private void consume(Model<?> model) { model.setState(state); }
    public Model<T> run() {
        Model<T> model = new Model<T>();
        consume(model);
        return model;
    }
}"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let trainer = translations
        .iter()
        .find(|translation| translation.path() == "Trainer.java")
        .expect("trainer translation")
        .rust();
    assert!(trainer.contains("pub fn consume"), "{trainer}");
    assert!(trainer.contains("model: ModelValue<"), "{trainer}");
    assert!(
        !trainer.contains("self.consume(&model)") && !trainer.contains("self.consume(&mut model)"),
        "owned emitted parameter must not receive a borrow: {trainer}"
    );
    assert!(
        trainer.contains("self.consume(std::clone::Clone::clone(&model)")
            || trainer.contains("self.consume(model.clone()"),
        "owned all-handle carrier arguments must preserve Java aliasing: {trainer}"
    );
}

#[test]
fn batch_inherited_handle_companion_uses_declared_child_receiver() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "Base.java".to_string(),
            r#"package model;
public class Base<T> {
    int value;
    public void bump(int amount) { value += amount; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Child.java".to_string(),
            r#"package model;
public class Child<T> extends Base<T> {
    public Child<T> copy() { return this; }
}"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Use.java".to_string(),
            r#"package model;
public class Use<T> {
    public void run(Child<T> child) {
        java.util.List<Child<T>> values = new java.util.ArrayList<Child<T>>();
        values.add(child);
        Child<T> copy = values.get(0).copy();
        copy.bump(3);
    }
}"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let use_module = translations
        .iter()
        .find(|translation| translation.path() == "Use.java")
        .expect("use translation")
        .rust();
    assert!(
        use_module.contains("Child::bump_with_outer")
            && !use_module.contains("Base::bump_with_outer"),
        "{use_module}"
    );
}

#[test]
fn batch_carrier_receiver_reuses_generic_method_clone_requirement() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "SampleObservation.java".to_string(),
            "package model; public abstract class SampleObservation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "GenericProduct.java".to_string(),
            r#"
package model;
public interface GenericProduct<O extends SampleObservation> { O generate(); }
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "GenericProductProvider.java".to_string(),
            r#"
package model;
import java.util.List;
public class GenericProductProvider<O extends SampleObservation> {
    List<GenericProduct<O>> products;
    public GenericProduct<O> product() { return products.get(0); }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "SpecializedGenericProductProvider.java".to_string(),
            r#"
package model;
public class SpecializedGenericProductProvider<O extends SampleObservation>
        extends GenericProductProvider<O> {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "ProductHolder.java".to_string(),
            r#"
package model;
public class ProductHolder<O extends SampleObservation> {
    GenericProductProvider<O> provider;
    public O take() { return provider.product().generate(); }
}
"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let holder = translations
        .iter()
        .find(|translation| translation.path() == "ProductHolder.java")
        .expect("holder translation")
        .rust();
    assert!(
        holder.contains("pub fn take(&self) -> O where O: Clone"),
        "{holder}"
    );
}

#[test]
fn batch_nullable_collection_constructor_parameter_and_stored_field_share_shape() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "CarrierRecord.java".to_string(),
            r#"
package example.carrier;

public class CarrierRecord {
    int value;
    void setValue(int value) { this.value = value; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierBlock.java".to_string(),
            r#"
package example.carrier;

import java.util.List;

public class CarrierBlock {
    private List<CarrierRecord> originalRecords;

    public CarrierBlock(List<CarrierRecord> originalRecords) {
        this.originalRecords = originalRecords;
    }

    public List<CarrierRecord> takeRecords() {
        List<CarrierRecord> result = originalRecords;
        originalRecords = null;
        return result;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "CarrierProducer.java".to_string(),
            r#"
package example.carrier;

import java.util.ArrayList;
import java.util.List;

public class CarrierProducer {
    public CarrierBlock make() {
        List<CarrierRecord> records = new ArrayList<CarrierRecord>();
        CarrierRecord record = null;
        records.add(record);
        return new CarrierBlock(records);
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let block = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "CarrierBlock.java")
        .expect("carrier block translation");
    assert!(
        block.rust().contains(
            "original_records: Option<Vec<Option<std::rc::Rc<std::cell::RefCell<CarrierRecord>>>>>"
        ),
        "{}",
        block.rust()
    );
    assert!(
        block.rust().contains(
            "original_records: Vec<Option<std::rc::Rc<std::cell::RefCell<CarrierRecord>>>>)"
        ),
        "{}",
        block.rust()
    );
    cargo_check_generated_modules(
        "batch_nullable_collection_constructor_field_shape",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: block.rust().to_string(),
            tests: "",
        }],
    );
}

#[test]
fn batch_global_shapes_do_not_project_nullable_arguments_twice() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "ProjectionBatch.java".to_string(),
            r#"
interface ProjectionBlockData { int size(); }

interface ProjectionBlockSink {
    void accept(ProjectionBlockData value);
}

class ProjectionUncompressedBlockData implements ProjectionBlockData {
    ProjectionUncompressedBlockData peer;
    public int size() { return 1; }
}

class ProjectionBlock {
    ProjectionBlockData data;
    ProjectionUncompressedBlockData raw;
    ProjectionBlockData get() { return data; }
    ProjectionUncompressedBlockData getRaw() {
        if (raw == null) return null;
        return raw;
    }
    int useData(ProjectionBlockData value) { return value == null ? 0 : value.size(); }
}

class ProjectionRecord {
    String name;
    String getName() { return name; }
}

public class ProjectionBatch {
    ProjectionBlockSink sink;

    int forward(ProjectionBlock target, ProjectionBlock source) {
        ProjectionBlockData data = source.get();
        return target.useData(data) + target.useData(source.getRaw());
    }

    void forwardOptional(ProjectionBlockData data) {
        if (data != null)
            sink.accept(data);
    }

    int delimiterIndex(String name, Character delimiter) {
        return name.indexOf(delimiter);
    }

    String firstName(ProjectionRecord[] records) {
        return records[0].getName();
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "ProjectionBatchPeer.java".to_string(),
            "class ProjectionBatchPeer { int value() { return 1; } }".to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let projection = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "ProjectionBatch.java")
        .expect("projection batch translation");
    assert!(
        !projection
            .rust()
            .contains("Option<&&dyn ProjectionBlockData>")
            && !projection
                .rust()
                .contains(").as_ref() as &dyn ProjectionBlockData")
            && !projection.rust().contains("?.as_ref().cloned()")
            && !projection.rust().contains("?.get_name()")
            && !projection
                .rust()
                .contains("data.as_ref() as &dyn ProjectionBlockData"),
        "{}",
        projection.rust()
    );
    assert!(
        projection.rust().contains(
            ".get_raw().as_ref().map(|value| &**value).map(|value| value as &dyn ProjectionBlockData)"
        ),
        "nullable shared implementors must project the handle payload before the trait upcast: {}",
        projection.rust()
    );
    cargo_check_generated_modules(
        "batch_global_shapes_do_not_project_twice",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: projection.rust().to_string(),
            tests: "",
        }],
    );
}

#[test]
fn batch_nested_subclass_return_and_cast_select_source_hierarchy_carrier() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "BaseStream.java".to_string(),
            r#"
package example.streams;

public class BaseStream {
    int baseValue;
    BaseStream(int value) { baseValue = value; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "StreamFactory.java".to_string(),
            r#"
package example.streams;

public class StreamFactory implements StreamService {
    public BaseStream make() {
        ChildStream child = new ChildStream(7);
        return child;
    }
    public int read(BaseStream stream) { return ((ChildStream) stream).childValue; }
    public static BaseStream makeFallible() throws Exception {
        return new ChildStream(9);
    }

    private static class ChildStream extends BaseStream {
        int childValue;
        ChildStream(int value) { super(value); childValue = value + 1; }
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "DirectStreamFactory.java".to_string(),
            r#"
package example.streams;

public class DirectStreamFactory implements StreamService {
    public BaseStream make() {
        BaseStream base = new BaseStream(3);
        return base;
    }
    public int read(BaseStream stream) { return stream.baseValue; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "StreamService.java".to_string(),
            r#"
package example.streams;

public interface StreamService {
    BaseStream make();
    int read(BaseStream stream);
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "StreamUse.java".to_string(),
            r#"
package example.streams;

public class StreamUse {
    public BaseStream load() throws Exception {
        BaseStream stream = StreamFactory.makeFallible();
        return stream;
    }
}
"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let rendered = translations
        .iter()
        .map(|translation| translation.rust())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("pub enum BaseStreamValue")
            && rendered.contains("impl From<StreamFactory_ChildStream> for BaseStreamValue")
            && rendered.contains(
                "let base: BaseStreamValue = BaseStreamValue::BaseStream(BaseStream::new(3))"
            )
            && !rendered
                .contains("match BaseStream::new(3) { BaseStreamValue::BaseStream(value) => value"),
        "{rendered}"
    );
    let use_module = translations
        .iter()
        .find(|translation| translation.path() == "StreamUse.java")
        .expect("stream use translation")
        .rust();
    assert!(
        use_module.contains("let stream: BaseStreamValue = StreamFactory::make_fallible()?")
            && !use_module
                .contains("BaseStreamValue::BaseStream(StreamFactory::make_fallible()?)?"),
        "cross-file calls must retain the emitted carrier ABI instead of wrapping its Result payload twice: {use_module}"
    );
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "new_java2rust_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_batch_sources(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("A.java"),
        r#"
package demo;

public class A {
    public int inc(int value) {
        return value + 1;
    }

    public int callB() {
        return new B().two();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("B.java"),
        r#"
package demo;

public class B {
    public int two() {
        return 2;
    }
}
"#,
    )
    .unwrap();
}

#[test]
fn batch_timed_queue_offer_uses_declared_generic_field_element() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "FutureSink.java".to_string(),
            r#"
import java.util.concurrent.Future;

abstract class FutureSink {
    abstract void put(Future<BatchBlock> future) throws Exception;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "ThreadedFutureSink.java".to_string(),
            r#"
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

class ThreadedFutureSink extends FutureSink {
    private ArrayBlockingQueue<Future<BatchBlock>> queue;

    void put(Future<BatchBlock> future) throws Exception {
        while (!queue.offer(future, 100, TimeUnit.MILLISECONDS)) {}
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "BatchBlock.java".to_string(),
            r#"
import java.util.concurrent.Future;

class BatchBlock implements Future<BatchBlock> {
    public BatchBlock get() { return this; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "NullableFutureUse.java".to_string(),
            r#"
class NullableFutureUse {
    void sendNull(FutureSink sink) throws Exception { sink.put(null); }
}
"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let threaded = translations
        .iter()
        .find(|translation| translation.path() == "ThreadedFutureSink.java")
        .expect("threaded sink translation");
    assert!(
        threaded.rust().contains(
            "let __java_array_blocking_queue_timed_offer_value = std::clone::Clone::clone(&future).ok_or_else"
        ) || threaded.rust().contains(
            "let __java_array_blocking_queue_timed_offer_value = future.clone().ok_or_else"
        ),
        "{}",
        threaded.rust()
    );
    assert!(
        !threaded
            .rust()
            .contains("let __java_array_blocking_queue_timed_offer_value = Box::new"),
        "{}",
        threaded.rust()
    );
}

#[test]
fn batch_fallible_nullable_source_call_is_not_wrapped_twice() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "NullableFuturePipeline.java".to_string(),
            r#"
import java.util.concurrent.Future;

abstract class NullableFuturePipeline {
    static NullableFuturePipeline make(boolean first) {
        return first ? new NullableFuturePipelineImpl() : new NullableFuturePipelineImpl2();
    }
    abstract Future<NullableFutureValue> submit(NullableFutureWork work) throws Exception;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "NullableFuturePipelineImpl2.java".to_string(),
            r#"
import java.util.concurrent.Future;

class NullableFuturePipelineImpl2 extends NullableFuturePipeline {
    Future<NullableFutureValue> submit(NullableFutureWork work) throws Exception {
        return null;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "NullableFuturePipelineImpl.java".to_string(),
            r#"
import java.util.concurrent.Future;

class NullableFuturePipelineImpl extends NullableFuturePipeline {
    Future<NullableFutureValue> submit(NullableFutureWork work) throws Exception {
        NullableFutureValue value = work.process();
        return value;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "NullableFutureCallUse.java".to_string(),
            r#"
import java.util.concurrent.Future;

class NullableFutureCallUse {
    void run(NullableFutureWork work) throws Exception {
        NullableFuturePipeline pipeline = NullableFuturePipeline.make(true);
        Future<NullableFutureValue> future = pipeline.submit(work);
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "NullableFutureTypes.java".to_string(),
            r#"
class NullableFutureWork {
    NullableFutureValue process() { return null; }
}
class NullableFutureValue implements java.util.concurrent.Future<NullableFutureValue> {
    int value;
    void setValue(int value) { this.value = value; }
    public NullableFutureValue get() { return this; }
}
"#
            .to_string(),
        ),
    ];

    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let caller = translations
        .iter()
        .find(|translation| translation.path() == "NullableFutureCallUse.java")
        .expect("nullable future caller translation");
    assert!(
        caller.rust().contains("let future: Option<Future<")
            && caller.rust().contains(" = pipeline.submit(")
            && !caller.rust().contains("Some(pipeline.submit("),
        "{}",
        caller.rust()
    );
}

#[test]
fn batch_null_compared_shared_source_parameter_keeps_optional_carrier() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "BatchNullableRecord.java".to_string(),
            r#"
class BatchNullableRecord {
    int value;
    int take() { return value++; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "BatchNullableRecordQueue.java".to_string(),
            r#"
import java.util.ArrayDeque;
class BatchNullableRecordQueue {
    ArrayDeque<BatchNullableRecord> records = new ArrayDeque<BatchNullableRecord>();
    BatchNullableRecord poll() { return records.poll(); }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "BatchNullableValidator.java".to_string(),
            r#"
class BatchNullableValidator {
    BatchNullableRecord current;

    public int read(BatchNullableRecord record) {
        if (record != null) return record.take();
        return 0;
    }

    public BatchNullableRecord touch(BatchNullableRecord record) {
        if (record != null) record.take();
        return record;
    }

    public void touchCurrent() {
        if (current != null) current.take();
    }

    public int readNext(boolean present, BatchNullableRecord record) {
        BatchNullableRecord next = present ? record : null;
        return read(next);
    }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let validator = translations
        .iter()
        .find(|translation| translation.path() == "BatchNullableValidator.java")
        .expect("translated validator");

    assert!(
        validator.rust().contains("record: Option<")
            && validator.rust().contains("if record.is_some()")
            && validator.rust().contains("self.read(next)")
            && !validator.rust().contains("self.read(next.ok_or_else")
            && validator.rust().contains("record.as_ref().ok_or_else")
            && !validator.rust().contains("record.unwrap().clone()")
            && !validator
                .rust()
                .contains("__java_outer_receiver = self.current.clone();")
            && !validator
                .rust()
                .contains("unsupported Java null comparison"),
        "{}",
        validator.rust()
    );
}

fn generated_source_items_from_marker(rust: &str, marker: &str) -> String {
    let start = rust
        .find(marker)
        .unwrap_or_else(|| panic!("missing generated source marker {marker:?}:\n{rust}"));
    rust[start..].to_string()
}

fn generated_source_item_marker_count(rust: &str, marker: &str) -> usize {
    rust.matches(marker).count()
}

#[test]
fn batch_parallel_emission_matches_serial_output_order() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "A.java".to_string(),
            r#"
public class A {
    public int inc(int value) {
        return value + 1;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "B.java".to_string(),
            r#"
public class B {
    public int two() {
        return 2;
    }
}
"#
            .to_string(),
        ),
    ];

    let serial =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(1, None),
        );
    let parallel =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    assert_eq!(serial.translations().len(), parallel.translations().len());
    assert_eq!(serial.events().len(), parallel.events().len());
    for (serial, parallel) in serial.translations().iter().zip(parallel.translations()) {
        assert_eq!(serial.path(), parallel.path());
        assert_eq!(serial.rust(), parallel.rust());
        assert_eq!(serial.diagnostics().len(), parallel.diagnostics().len());
    }
    assert_eq!(
        parallel
            .events()
            .iter()
            .map(|event| (event.phase(), event.index(), event.status()))
            .collect::<Vec<_>>(),
        vec![
            ("emit_start", 1, "running"),
            ("emit_end", 1, "ok"),
            ("emit_start", 2, "running"),
            ("emit_end", 2, "ok")
        ]
    );
}

#[test]
fn batch_flattened_nested_enum_values_keeps_instance_loop_binding() {
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &[new_java2rust::BatchSource::new(
                "EnumOwner.java",
                r#"
class EnumOwner {
    enum Arguments {
        FIRST, SECOND;
        boolean set;
        void setSet(boolean value) { set = value; }
    }

    static void reset() {
        for (Arguments arg : Arguments.values()) {
            arg.setSet(false);
        }
    }
}
"#,
            )],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let rust = report.translations()[0].rust();
    assert!(
        rust.contains("EnumOwner_Arguments::values().iter().copied()")
            && rust.contains("arg.set_set(false)")
            && !rust.contains("Arg::set_set(false)"),
        "{rust}"
    );
}

#[test]
fn batch_translation_uses_shared_local_inner_class_construction() {
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[new_java2rust::BatchSource::new(
                "MarkerInnerCapture.java".to_string(),
                r#"
public class MarkerInnerCapture {
    private int base = 10;
    class Adder {
        int add(int x) { return base + x; }
    }
    public int run() {
        Adder a = new Adder();
        base = 20;
        return a.add(1);
    }
}
"#
                .to_string(),
            )],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );

    assert_eq!(translations.len(), 1);
    assert!(
        !translations[0]
            .diagnostics()
            .has_code("inner_class_construction_panics"),
        "SharedLocal construction must not retain the obsolete inner-class \
         panic diagnostic:\n{:?}",
        translations[0].diagnostics().iter().collect::<Vec<_>>()
    );
}

#[test]
fn batch_duplicate_simple_type_names_keep_distinct_signature_keys() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/DuplicateOwner.java".to_string(),
            r#"
package a;

public class DuplicateOwner {
    private int first;

    public int firstValue() {
        return first;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/DuplicateOwner.java".to_string(),
            r#"
package b;

public class DuplicateOwner {
    private int second;

    public int secondValue() {
        return second;
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    assert_eq!(report.translations().len(), 2);
    assert!(
        report.translations()[0].rust().contains("first_value")
            && report.translations()[1].rust().contains("second_value"),
        "{:#?}",
        report
            .translations()
            .iter()
            .map(|translation| (translation.path(), translation.rust()))
            .collect::<Vec<_>>()
    );
    cargo_check_generated_modules(
        "batch_duplicate_simple_type_names",
        report
            .translations()
            .iter()
            .enumerate()
            .map(|(index, translation)| GeneratedCrateModule {
                name: if index == 0 { "a_owner" } else { "b_owner" },
                rust: translation.rust().to_string(),
                tests: "",
            })
            .collect(),
    );
}

#[test]
fn explicit_import_aliases_survive_duplicate_context_simple_names() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Cfg.java".to_string(),
            r#"
package a;

public class Cfg {
    public static final int N = 10;

    public static int who() {
        return 1;
    }

    public int id() {
        return 100;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Cfg.java".to_string(),
            r#"
package b;

public class Cfg {
    public static final int N = 20;

    public static int who() {
        return 2;
    }

    public int id() {
        return 200;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "UseA.java".to_string(),
            r#"
import a.Cfg;

public class UseA {
    public static int run() {
        Cfg c = new Cfg();
        return Cfg.N + Cfg.who() + c.id();
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "UseB.java".to_string(),
            r#"
import b.Cfg;

public class UseB {
    public static int run() {
        Cfg c = new Cfg();
        return Cfg.N + Cfg.who() + c.id();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let use_a = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "UseA.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let use_b = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "UseB.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(
        use_a.rust().contains("ACfg::N")
            && use_a.rust().contains("ACfg::who()")
            && !use_a.rust().contains("BCfg::N")
            && !use_a.rust().contains("BCfg::who()"),
        "{}",
        use_a.rust()
    );
    assert!(
        use_b.rust().contains("BCfg::N")
            && use_b.rust().contains("BCfg::who()")
            && !use_b.rust().contains("ACfg::N")
            && !use_b.rust().contains("ACfg::who()"),
        "{}",
        use_b.rust()
    );
}

#[test]
fn package_qualified_source_type_references_keep_package_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {
    public static final int KIND = 1;

    public int who() {
        return 1;
    }
}

"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {
    public static final int KIND = 2;

    public int who() {
        return 2;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static int run() {
        a.Item ai = new a.Item();
        b.Item bi = new b.Item();
        return ai.who() + bi.who() + a.Item.KIND + b.Item.KIND;
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(main.rust().contains("AItem::new()"), "{}", main.rust());
    assert!(main.rust().contains("BItem::new()"), "{}", main.rust());
    assert!(main.rust().contains("AItem::KIND"), "{}", main.rust());
    assert!(main.rust().contains("BItem::KIND"), "{}", main.rust());
    assert!(!main.rust().contains("let ai: Item"), "{}", main.rust());
    assert!(!main.rust().contains("let bi: Item"), "{}", main.rust());
    assert!(!main.rust().contains("= Item::new()"), "{}", main.rust());
    assert!(
        a_item.rust().contains("pub struct AItem"),
        "{}",
        a_item.rust()
    );
    assert!(
        b_item.rust().contains("pub struct BItem"),
        "{}",
        b_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&a_item.rust(), "pub struct AItem"),
        1,
        "{}",
        a_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&b_item.rust(), "pub struct BItem"),
        1,
        "{}",
        b_item.rust()
    );
    assert_eq!(
        generated_source_item_marker_count(&a_item.rust(), "pub struct BItem")
            + generated_source_item_marker_count(&b_item.rust(), "pub struct AItem"),
        0,
        "package-peer emitted items should not cross owners:\n{}\n{}",
        a_item.rust(),
        b_item.rust()
    );
    assert!(
        !a_item.rust().contains("pub type Item = AItem;"),
        "{}",
        a_item.rust()
    );
    assert!(
        !b_item.rust().contains("pub type Item = BItem;"),
        "{}",
        b_item.rust()
    );
    assert!(
        !a_item.rust().contains("pub struct Item") && !b_item.rust().contains("pub struct Item"),
        "declarations should not collapse to the simple name:\n{}\n{}",
        a_item.rust(),
        b_item.rust()
    );

    cargo_check_generated_modules(
        "batch_same_simple_package_peers",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub struct JavaClass<T> {{ name: &'static str, _marker: std::marker::PhantomData<T> }}\n\
                 impl<T> JavaClass<T> {{ pub fn literal(name: &'static str) -> Self {{ Self {{ name, _marker: std::marker::PhantomData }} }} pub fn get_name(&self) -> String {{ self.name.to_string() }} }}\n\
                 pub fn java_erased_object_to_string<T>(_value: &T) -> String {{ panic!(\"unsupported Java erased object display: runtime target is erased\") }}\n\
                 {}\n{}\n{}",
                generated_source_items_from_marker(&a_item.rust(), "pub struct AItem"),
                generated_source_items_from_marker(&b_item.rust(), "pub struct BItem"),
                generated_source_items_from_marker(&main.rust(), "pub struct Main")
            ),
            tests: "",
        }],
    );
}

#[test]
fn imported_unique_source_static_owner_uses_its_emitted_declaration_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "jaligner/util/Commons.java".to_string(),
            r#"
package jaligner.util;

public abstract class Commons {
    private static String lineSeparator = "\n";
    private static boolean jnlp = false;
    public static String getLineSeparator() { return lineSeparator; }
    public static void setJnlp(boolean value) { Commons.jnlp = value; }
    static { setJnlp(jnlp); }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "jaligner/Alignment.java".to_string(),
            r#"
package jaligner;

import jaligner.util.Commons;

public class Alignment {
    public String run() {
        StringBuffer buffer = new StringBuffer();
        buffer.append(Commons.getLineSeparator());
        return buffer.toString();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let alignment = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "jaligner/Alignment.java")
        .expect("translated Alignment source");
    let commons = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "jaligner/util/Commons.java")
        .expect("translated Commons source");

    assert!(
        commons.rust().contains("pub struct Commons"),
        "{}",
        commons.rust()
    );
    assert!(
        commons.rust().contains("Self::__set_static_jnlp(value);")
            && commons
                .rust()
                .contains("Self::set_jnlp(Self::__static_jnlp());"),
        "{}",
        commons.rust()
    );
    assert!(
        alignment.rust().contains("Commons::get_line_separator()")
            && !alignment
                .rust()
                .contains("JalignerCommons::get_line_separator()"),
        "{}",
        alignment.rust()
    );
}

#[test]
fn duplicate_static_field_names_resolve_to_the_current_source_declaration() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/First.java".to_string(),
            r#"
package a;
import java.util.logging.Level;
import java.util.logging.Logger;

public class First {
    private static final Logger logger = Logger.getLogger(First.class.getName());
    public static void report(Exception error) {
        logger.log(Level.WARNING, "first", error);
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Second.java".to_string(),
            r#"
package b;
import java.util.logging.Level;
import java.util.logging.Logger;

public class Second {
    private static final Logger logger = Logger.getLogger(Second.class.getName());
    public static void report(Exception error) {
        logger.log(Level.WARNING, "second", error);
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    for (path, owner, other) in [
        (
            "a/First.java",
            "Self::__static_logger().log_3(",
            "SECOND_LOGGER",
        ),
        (
            "b/Second.java",
            "Self::__static_logger().log_3(",
            "FIRST_LOGGER",
        ),
    ] {
        let translation = report
            .translations()
            .iter()
            .find(|translation| translation.path() == path)
            .expect("translated source");
        assert!(translation.rust().contains(owner), "{}", translation.rust());
        assert!(
            !translation.rust().contains(other),
            "{}",
            translation.rust()
        );
    }
}

#[test]
fn throwing_argument_does_not_make_infallible_cross_file_constructor_fallible() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "Value.java".to_string(),
            "public class Value { public Value(String text) {} }".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Factory.java".to_string(),
            r#"
import java.io.IOException;
public class Factory {
    String load() throws IOException { throw new IOException(); }
    Value make() throws IOException { return new Value(load()); }
}

"#
            .to_string(),
        ),
    ];
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let factory = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Factory.java")
        .expect("translated Factory source");
    assert!(
        factory.rust().contains("Value::new(self.load()?)")
            && !factory.rust().contains("Value::new(self.load()?)?"),
        "{}",
        factory.rust()
    );
}

#[test]
fn emitted_constructor_fallibility_wins_over_duplicate_source_summary() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "FallibleSummaryValue.java".to_string(),
            r#"
public class FallibleSummaryValue {
    int value;
    String name;
    String sequence;
    String comment;
    String quality;
    public FallibleSummaryValue(String name, String sequence, String comment, String quality, int offset) {
        this.name = name;
        this.sequence = sequence;
        this.comment = comment;
        this.quality = quality;
        if (sequence.length() != quality.length()) throw new RuntimeException("mismatch");
        value = offset;
    }
    public FallibleSummaryValue(int value) { this.value = value; }
    public void setValue(int value) { this.value = value; }
    public static FallibleSummaryValue create(String text) {
        StringBuilder builder = new StringBuilder();
        for (int i = 0; i < text.length(); i++) builder.append('I');
        return new FallibleSummaryValue(text, text, text, builder.toString(), 33);
    }
    public static int run(String text) {
        FallibleSummaryValue first = create(text);
        FallibleSummaryValue alias = first;
        alias.setValue(9);
        return first.value;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "FallibleSummaryOwner.java".to_string(),
            r#"
public class FallibleSummaryOwner {
    FallibleSummaryValue current;
    public void make(String text) {
        current = new FallibleSummaryValue(text);
    }
}
"#
            .to_string(),
        ),
    ];
    let summary_sources = vec![
        r#"
public class FallibleSummaryValue {
    int value;
    String name;
    String sequence;
    String comment;
    String quality;
    public FallibleSummaryValue(String name, String sequence, String comment, String quality, int offset) {
        this.name = name;
        this.sequence = sequence;
        this.comment = comment;
        this.quality = quality;
        if (sequence.length() != quality.length()) throw new RuntimeException("mismatch");
        value = offset;
    }
    public FallibleSummaryValue(int value) { this.value = value; }
    public void setValue(int value) { this.value = value; }
    public static FallibleSummaryValue create(String text) {
        StringBuilder builder = new StringBuilder();
        for (int i = 0; i < text.length(); i++) builder.append('I');
        return new FallibleSummaryValue(text, text, text, builder.toString(), 33);
    }
    public static int run(String text) {
        FallibleSummaryValue first = create(text);
        FallibleSummaryValue alias = first;
        alias.setValue(9);
        return first.value;
    }
}
"#
        .to_string(),
        r#"
public class FallibleSummaryOwner {
    FallibleSummaryValue current;
    public void make(String text) {
        current = new FallibleSummaryValue(text);
    }
}
"#
        .to_string(),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let owner = translations
        .iter()
        .find(|translation| translation.path() == "FallibleSummaryOwner.java")
        .expect("translated owner");
    let value = translations
        .iter()
        .find(|translation| translation.path() == "FallibleSummaryValue.java")
        .expect("translated value");

    assert!(
        owner.rust().contains("FallibleSummaryValue::new_") && owner.rust().contains(")?"),
        "{}",
        owner.rust()
    );
    assert!(
        !owner
            .rust()
            .contains("FallibleSummaryValue::new(Some(&text)))"),
        "{}",
        owner.rust()
    );
    assert!(
        value.rust().contains(
            "std::rc::Rc::new(std::cell::RefCell::new(FallibleSummaryValue::new_string_string_string_string_int("
        ) && value.rust().contains(", 33)?))"),
        "{}",
        value.rust()
    );
}

#[test]
fn packaged_emitted_constructor_fallibility_wins_over_duplicate_summary() {
    let source = r#"
package org.usadellab.trimmomatic.fastq;

public class FastqRecord {
    String name;
    String sequence;
    String comment;
    String quality;
    int value;

    public FastqRecord(String name, String sequence, String comment, String quality, int offset) {
        this.name = name;
        this.sequence = sequence;
        this.comment = comment;
        this.quality = quality;
        if (sequence.length() != quality.length()) throw new RuntimeException("mismatch");
        value = offset;
    }

    public void setValue(int value) { this.value = value; }

    public static FastqRecord make(String name, String sequence, int quality) {
        StringBuilder builder = new StringBuilder();
        for (int i = 0; i < sequence.length(); i++) builder.append('I');
        return new FastqRecord(name, sequence, name, builder.toString(), 33);
    }

    public static int run(String name, String sequence) {
        FastqRecord first = make(name, sequence, 40);
        FastqRecord alias = first;
        alias.setValue(9);
        return first.value;
    }
}
"#;
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "org/usadellab/trimmomatic/fastq/FastqRecord.java".to_string(),
        source.to_string(),
    )];
    let summary_sources = vec![source.to_string()];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let record = translations
        .iter()
        .find(|translation| {
            translation.path() == "org/usadellab/trimmomatic/fastq/FastqRecord.java"
        })
        .expect("translated record");

    assert!(
        record
            .rust()
            .contains("std::rc::Rc::new(std::cell::RefCell::new(FastqRecord::new(")
            && record.rust().contains(", 33)?))"),
        "{}",
        record.rust()
    );
}

#[test]
fn duplicate_source_summary_atomic_field_get_keeps_infallible_facade_abi() {
    let atomic_source = r#"
import java.util.concurrent.atomic.AtomicBoolean;
public class SummaryAtomicOwner {
    AtomicBoolean flag;
    public SummaryAtomicOwner(boolean present) {
        flag = present ? new AtomicBoolean(false) : null;
    }
    public boolean value() { return flag.get(); }
}
"#;
    let fallible_get_source = r#"
public class SummaryFallibleGet {
    public Object get() throws Exception { throw new Exception("failed"); }
}
"#;
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "SummaryAtomicOwner.java".to_string(),
            atomic_source.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "SummaryFallibleGet.java".to_string(),
            fallible_get_source.to_string(),
        ),
    ];
    let summary_sources = vec![atomic_source.to_string(), fallible_get_source.to_string()];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let owner = translations
        .iter()
        .find(|translation| translation.path() == "SummaryAtomicOwner.java")
        .expect("translated atomic owner");

    assert!(owner.rust().contains(".get();"), "{}", owner.rust());
    assert!(
        !owner.rust().contains(".get().unwrap_or_else"),
        "{}",
        owner.rust()
    );
}

#[test]
fn duplicate_source_summary_nullable_string_adapts_at_pattern_matcher_boundary() {
    let pattern_source = r#"
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public enum SummaryNamePattern {
    ALPHA("alpha", "A"), BETA("beta", "B");

    private Pattern pattern;
    private String replacement;

    SummaryNamePattern(String expression, String replacement) {
        this.pattern = Pattern.compile(expression);
        this.replacement = replacement;
    }

    public String canonicalizeOne(String value) {
        String candidate = value.length() > 0 ? value : null;
        Matcher matcher = pattern.matcher(candidate);
        if (!matcher.find()) return null;
        return matcher.replaceAll(replacement);
    }

    public static String canonicalize(String value) {
        for (SummaryNamePattern candidate : SummaryNamePattern.values()) {
            String canonical = candidate.canonicalizeOne(value);
            if (canonical != null) return canonical;
        }
        return null;
    }
}
"#;
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "SummaryNamePattern.java".to_string(),
        pattern_source.to_string(),
    )];
    let summary_sources = vec![pattern_source.to_string()];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let pattern = translations
        .iter()
        .find(|translation| translation.path() == "SummaryNamePattern.java")
        .expect("translated pattern enum");

    assert!(
        pattern.rust().contains("candidate: Option")
            && pattern.rust().contains(".matcher(candidate.as_ref()"),
        "{}",
        pattern.rust()
    );
}

#[test]
fn summary_source_constructor_slot_uses_external_stream_conversion_adapter() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "example/use/StreamHolderUse.java".to_string(),
        r#"
package example.use;
import java.io.FileInputStream;
import example.util.StreamHolder;

public class StreamHolderUse {
    public static StreamHolder open(String path) throws Exception {
        return new StreamHolder(new FileInputStream(path), 1L);
    }
}
"#
        .to_string(),
    )];
    let summary_sources = vec![
        r#"
package example.util;
import java.io.InputStream;
import java.io.IOException;

public class StreamHolder extends InputStream {
    public StreamHolder(InputStream stream, long length) {}
    public int read() throws IOException { return -1; }
}
"#
        .to_string(),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let use_site = translations
        .iter()
        .find(|translation| translation.path() == "example/use/StreamHolderUse.java")
        .expect("translated stream holder use");

    assert!(
        use_site.rust().contains("FileInputStream::new(path)?")
            && !use_site.rust().contains("as InputStream"),
        "{}",
        use_site.rust()
    );
}

#[test]
fn imported_summary_static_owner_uses_emitted_declaration_identity() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "org/usadellab/trimmomatic/fastq/Parser.java".to_string(),
        r#"
package org.usadellab.trimmomatic.fastq;
import java.io.InputStream;
import org.usadellab.trimmomatic.util.compression.CompressionFormat;

public class Parser {
    public static InputStream open(InputStream stream, String name) throws Exception {
        return CompressionFormat.wrapStreamForParsing(stream, name);
    }
    public static InputStream save(InputStream stream, String name) throws Exception {
        return CompressionFormat.wrapStreamForSerializing(stream, name, null);
    }
}

"#
        .to_string(),
    )];
    let summary_sources = vec![
        r#"
package org.usadellab.trimmomatic.util.compression;
import java.io.InputStream;

public enum CompressionFormat {
    NONE("");
    private String ext;
    CompressionFormat(String ext) { this.ext = ext; }
    public static InputStream wrapStreamForParsing(InputStream stream, String name) {
        return stream;
    }
    public static InputStream wrapStreamForSerializing(InputStream stream, String name, Integer level) {
        return stream;
    }
}
"#
        .to_string(),
        r#"
package org.usadellab.trimmomatic.fastq;
import java.io.InputStream;
import org.usadellab.trimmomatic.util.compression.CompressionFormat;

public class Parser {
    public static InputStream open(InputStream stream, String name) throws Exception {
        return CompressionFormat.wrapStreamForParsing(stream, name);
    }
    public static InputStream save(InputStream stream, String name) throws Exception {
        return CompressionFormat.wrapStreamForSerializing(stream, name, null);
    }
}
"#
        .to_string(),
        r#"
package unrelated.context;

public class Noise {
    int compressionFormat;
}
"#
        .to_string(),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let parser = translations
        .iter()
        .find(|translation| translation.path() == "org/usadellab/trimmomatic/fastq/Parser.java")
        .expect("translated parser");

    assert!(
        parser
            .rust()
            .contains("CompressionFormat::wrap_stream_for_parsing(")
            && !parser.rust().contains(
                "OrgUsadellabTrimmomaticFastqCompressionFormat::wrap_stream_for_parsing("
            )
            && !parser.rust().contains(
                "OrgUsadellabTrimmomaticUtilCompressionCompressionFormat::wrap_stream_for_parsing("
            )
            && parser
                .rust()
                .contains("wrap_stream_for_serializing(stream, name, None::<i32>)")
            && parser.rust().contains("_arg2: Option<i32>"),
        "{}",
        parser.rust()
    );
}

#[test]
fn nullable_shared_summary_subclass_does_not_fabricate_external_super_conversion() {
    let parser_source = r#"
package example.fastq;
import java.io.InputStream;
import example.util.PositionTrackingInputStream;
import example.util.CompressionFormat;

public class Parser {
    PositionTrackingInputStream tracked;

    public InputStream open(InputStream stream, String name) throws Exception {
        tracked = new PositionTrackingInputStream(stream, 1L);
        return CompressionFormat.wrap(tracked, name);
    }
}
"#;
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "example/fastq/Parser.java".to_string(),
        parser_source.to_string(),
    )];
    let summary_sources = vec![
        parser_source.to_string(),
        r#"
package example.util;
import java.io.InputStream;

public class PositionTrackingInputStream extends InputStream {
    public PositionTrackingInputStream(InputStream stream, long length) {}
    public int read() { return -1; }
}
"#
        .to_string(),
        r#"
package example.util;
import java.io.InputStream;

public class CompressionFormat {
    public static InputStream wrap(InputStream stream, String name) throws Exception {
        return stream;
    }
}
"#
        .to_string(),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let parser = translations
        .iter()
        .find(|translation| translation.path() == "example/fastq/Parser.java")
        .expect("translated parser");
    let wrap_args = parser
        .rust()
        .split("ExampleUtilCompressionFormat::wrap(")
        .nth(1)
        .and_then(|tail| tail.split_once(", name)"))
        .map(|(args, _)| args)
        .expect("generated CompressionFormat.wrap arguments");
    let constructor_signatures = parser
        .rust()
        .lines()
        .filter(|line| line.contains("pub fn new"))
        .collect::<Vec<_>>()
        .join("\n");
    let has_compile_error = wrap_args.contains("compile_error!");
    let has_upcast_diagnostic = wrap_args.contains("unsupported Java class upcast");
    let has_fabricated_conversion = parser
        .rust()
        .contains("impl From<std::rc::Rc<PositionTrackingInputStream>> for InputStream");
    let has_summary_constructor = parser
        .rust()
        .contains("pub fn new(_arg0: InputStream, _arg1: i64) -> Self");
    assert!(
        has_compile_error
            && has_upcast_diagnostic
            && !has_fabricated_conversion
            && has_summary_constructor,
        "compile_error={has_compile_error} upcast_diagnostic={has_upcast_diagnostic} fabricated_conversion={has_fabricated_conversion} summary_constructor={has_summary_constructor}\nwrap args: {wrap_args}\nconstructors:\n{constructor_signatures}"
    );
}

#[test]
fn emitted_same_package_static_owner_beats_qualified_summary_copy() {
    let pattern = r#"
package org.usadellab.trimmomatic.fastq;

public class RecordNamePattern {
    public static String canonicalize(String value) { return value; }
}
"#;
    let validator = r#"
package org.usadellab.trimmomatic.fastq;

public class PairingValidator {
    public static String validate(String value) {
        return RecordNamePattern.canonicalize(value);
    }
}
"#;
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "org/usadellab/trimmomatic/fastq/RecordNamePattern.java".to_string(),
            pattern.to_string(),
        ),
        new_java2rust::BatchSource::new(
            "org/usadellab/trimmomatic/fastq/PairingValidator.java".to_string(),
            validator.to_string(),
        ),
    ];
    let summary_sources = vec![pattern.to_string(), validator.to_string()];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &summary_sources,
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let validator = translations
        .iter()
        .find(|translation| {
            translation.path() == "org/usadellab/trimmomatic/fastq/PairingValidator.java"
        })
        .expect("translated validator");

    assert!(
        validator
            .rust()
            .contains("RecordNamePattern::canonicalize(")
            && !validator
                .rust()
                .contains("OrgUsadellabTrimmomaticFastqRecordNamePattern::canonicalize("),
        "{}",
        validator.rust()
    );
}

#[test]
fn late_mutable_nested_override_propagates_to_same_receiver_caller() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "example/Owner.java".to_string(),
        r#"
package example;

import java.util.zip.Deflater;

class Owner {
    class Base {
        void deflate(int mode) {}
        void write() { deflate(Deflater.NO_FLUSH); }
    }

    class Child extends Base {
        int count;
        @Override void deflate(int mode) { count += mode; }
    }
}
"#
        .to_string(),
    )];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let owner = translations
        .iter()
        .find(|translation| translation.path() == "example/Owner.java")
        .expect("translated owner");

    assert!(
        owner.rust().contains("pub fn write(&mut self)"),
        "{}",
        owner.rust()
    );
}

#[test]
fn batch_static_object_calls_reuse_declared_storage_accessor_in_every_method() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/LogMath.java".to_string(),
            r#"
package example;

class LogMath {
    static LogMath INST = new LogMath();
    double log(double value) { return Math.log(value); }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/LogMathUse.java".to_string(),
            r#"
package example;

class LogMathUse {
    LogMathUse() {
        accept(LogMath.INST.log(1.0));
    }

    double apply(double value) {
        ValueSink sink = new ValueSink();
        sink.accept(LogMath.INST.log(value));
        return LogMath.INST.log(sink.read());
    }

    void accept(double value) {}
}

class ValueSink {
    double value;
    void accept(double value) { this.value = value; }
    double read() { return value; }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let use_translation = translations
        .iter()
        .find(|translation| translation.path() == "example/LogMathUse.java")
        .expect("translated static object user");

    assert_eq!(
        use_translation
            .rust()
            .matches("LogMath::__static_i_n_s_t()")
            .count(),
        3,
        "{}",
        use_translation.rust()
    );
    assert!(
        !use_translation.rust().contains("LogMath::INST"),
        "{}",
        use_translation.rust()
    );
}

#[test]
fn batch_emitted_getter_return_shape_drives_required_argument_lowering() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "ContextNullableSource.java".to_string(),
            r#"
public class ContextNullableSource {
    String value;
    String getValue() { return value; }
    void setValue(String value) { this.value = value; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "ContextGetterConsumer.java".to_string(),
            r#"
public class ContextGetterConsumer {
    static int[] pack(String value) {
        int[] out = null;
        out = new int[value.length()];
        out[0] = value.charAt(0);
        return out;
    }
    class Inner {
        int run(ContextNullableSource source) {
            ContextNullableSource alias = source;
            alias.setValue(source.getValue());
            return pack(source.getValue())[0];
        }
    }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let consumer = translations
        .iter()
        .find(|translation| translation.path() == "ContextGetterConsumer.java")
        .expect("translated consumer");

    assert!(
        consumer.rust().contains("::pack(source")
            && consumer.rust().contains("get_value().as_ref().ok_or_else")
            && !consumer
                .rust()
                .contains("&source.borrow().get_value().as_ref()"),
        "{}",
        consumer.rust()
    );
}

#[test]
fn batch_outer_handle_call_materializes_shared_field_receiver_before_owner_arguments() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "CallOwner.java".to_string(),
            r#"
public class CallOwner {
    Sink sink;
    int count = 3;
    class Inner {}

    CallOwner(Sink sink) { this.sink = sink; }

    void run() {
        new Inner();
        sink.report(count);
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Sink.java".to_string(),
            r#"
public class Sink {
    void report(int value) {}
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let owner = translations
        .iter()
        .find(|translation| translation.path() == "CallOwner.java")
        .expect("translated owner");

    assert!(
        owner.rust().contains(
            "let __java_call_receiver = std::clone::Clone::clone(&__java_self.borrow().sink)"
        ) || owner
            .rust()
            .contains("let __java_call_receiver = __java_self.borrow().sink.clone()")
            || owner
                .rust()
                .contains("let __java_call_receiver = __java_self.borrow().sink.as_ref().cloned()"),
        "{}",
        owner.rust()
    );
    assert!(
        owner.rust().contains("__java_call_receiver")
            && (owner.rust().contains(".report(") || owner.rust().contains("Sink::report(")),
        "{}",
        owner.rust()
    );
}

#[test]
fn concrete_source_comparator_uses_fallible_sort_bridge_when_compare_lowers_to_result() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "LetterComparator.java".to_string(),
            r#"
import java.util.Comparator;
public class LetterComparator implements Comparator<String> {
    public int compare(String left, String right) {
        return left.charAt(0) - right.charAt(0);
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Sorter.java".to_string(),
            r#"
import java.util.ArrayList;
import java.util.Collections;
public class Sorter {
    public ArrayList<String> sort(ArrayList<String> values) {
        Collections.sort(values, new LetterComparator());
        return values;
    }
}
"#
            .to_string(),
        ),
    ];
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );
    let comparator = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "LetterComparator.java")
        .expect("translated comparator source");
    let sorter = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Sorter.java")
        .expect("translated sorter source");
    assert!(
        comparator.rust().contains(
            "pub fn compare(&self, left: &str, right: &str) -> Result<i32, JavaException>"
        ),
        "{}",
        comparator.rust()
    );
    assert!(
        sorter.rust().contains("java_collection_sort_by_result")
            && sorter
                .rust()
                .contains("__java_comparator.compare(a.clone(), b.clone())"),
        "{}",
        sorter.rust()
    );
}

#[test]
fn resolved_source_method_shape_wins_over_same_named_external_method_in_constructor_slot() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "jaligner/ui/filechooser/NamedInputStream.java".to_string(),
            r#"
package jaligner.ui.filechooser;
import java.io.Serializable;
import java.io.InputStream;
public class NamedInputStream implements Serializable {
    private String name = null;
    private InputStream inputStream = null;
    public NamedInputStream(String name, InputStream inputStream) {
        this.name = name;
        this.inputStream = inputStream;
    }
    public String getName() { return this.name; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "jaligner/matrix/Matrix.java".to_string(),
            "package jaligner.matrix; import java.io.Serializable; public class Matrix implements Serializable { public Matrix(String value, float[][] scores) {} }"
                .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "jaligner/matrix/MatrixLoader.java".to_string(),
            r#"
package jaligner.matrix;
import jaligner.ui.filechooser.NamedInputStream;
public class MatrixLoader {
    public static Matrix make(NamedInputStream nis, float[][] scores) {
        return new Matrix(nis.getName(), scores);
    }
}
"#
            .to_string(),
        ),
    ];
    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(3, None),
        );
    let holder = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "jaligner/ui/filechooser/NamedInputStream.java")
        .expect("translated holder source");
    let use_site = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "jaligner/matrix/MatrixLoader.java")
        .expect("translated use source");
    assert!(
        holder
            .rust()
            .contains("pub fn get_name(&self) -> Option<String>"),
        "{}",
        holder.rust()
    );
    assert!(
        use_site.rust().contains("nis.get_name().ok_or_else(")
            || use_site
                .rust()
                .contains("nis.get_name().expect(\"Java null receiver\")"),
        "{}",
        use_site.rust()
    );
}

#[test]
fn stream_static_constant_support_keeps_package_qualified_owner_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {
    public static final int KIND = 1;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {
    public static final int KIND = 2;
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static java.util.List<Integer> stream(java.util.List<Integer> xs) {
        return xs.stream().map(x -> a.Item.KIND + b.Item.KIND + x).toList();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    assert!(main.rust().contains("AItem::KIND"), "{}", main.rust());
    assert!(main.rust().contains("BItem::KIND"), "{}", main.rust());
    assert!(!main.rust().contains("pub struct Item;"), "{}", main.rust());
    assert!(
        !main.rust().contains("pub type Item = AItem;"),
        "{}",
        main.rust()
    );
    assert!(
        !main.rust().contains("pub type Item = BItem;"),
        "{}",
        main.rust()
    );
    assert!(
        !a_item.rust().contains("pub type Item = AItem;"),
        "{}",
        a_item.rust()
    );
    assert!(
        !b_item.rust().contains("pub type Item = BItem;"),
        "{}",
        b_item.rust()
    );
    assert!(
        !main.rust().contains("pub const KIND: KIND = KIND;"),
        "{}",
        main.rust()
    );
}

#[test]
fn imported_source_enum_value_of_keeps_package_owner_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Mode.java".to_string(),
            r#"
package a;

public enum Mode {
    FIRST,
    SECOND
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Mode.java".to_string(),
            r#"
package b;

public enum Mode {
    OTHER
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
import a.Mode;

public class Main {
    public static int run(String name) {
        return Mode.valueOf(name).ordinal();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_mode = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Mode.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_mode = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Mode.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));

    assert!(
        main.rust().contains("AMode::value_of(name.as_str())"),
        "{}",
        main.rust()
    );
    assert!(
        !main.rust().contains("return Mode::value_of("),
        "{}",
        main.rust()
    );
    assert!(!main.rust().contains("BMode::value_of("), "{}", main.rust());

    cargo_check_generated_modules(
        "batch_imported_source_enum_value_of_package_owner",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: format!(
                "pub struct JavaClass<T> {{ name: &'static str, _marker: std::marker::PhantomData<T> }}\n\
                 impl<T> JavaClass<T> {{ pub fn literal(name: &'static str) -> Self {{ Self {{ name, _marker: std::marker::PhantomData }} }} pub fn get_name(&self) -> String {{ self.name.to_string() }} }}\n\
                 pub trait JavaEnum: Clone + Eq + Ord + std::hash::Hash + std::fmt::Display {{ fn values() -> Vec<Self> where Self: Sized; fn ordinal(&self) -> i32; fn name(&self) -> &'static str; }}\n\
                 pub trait JavaAppendValue {{ fn into_java_append_string(self) -> String; }}\n\
                 pub fn java_erased_object_to_string<T>(_value: &T) -> String {{ panic!(\"unsupported Java erased object display: runtime target is erased\") }}\n\
                 {}\n{}\n{}",
                format!(
                    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\n{}",
                    generated_source_items_from_marker(&a_mode.rust(), "pub enum AMode")
                ),
                format!(
                    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\n{}",
                    generated_source_items_from_marker(&b_mode.rust(), "pub enum BMode")
                ),
                generated_source_items_from_marker(&main.rust(), "pub struct Main")
            ),
            tests: "",
        }],
    );
}

#[test]
fn package_qualified_class_literals_keep_package_identity() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "a/Item.java".to_string(),
            r#"
package a;

public class Item {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "b/Item.java".to_string(),
            r#"
package b;

public class Item {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "Main.java".to_string(),
            r#"
public class Main {
    public static String run() {
        return a.Item.class.getName() + "," + b.Item.class.getName();
    }
}
"#
            .to_string(),
        ),
    ];

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
            &new_java2rust::BatchTranslationOptions::new(2, None),
        );

    let main = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "Main.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let a_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "a/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let b_item = report
        .translations()
        .iter()
        .find(|translation| translation.path() == "b/Item.java")
        .unwrap_or_else(|| panic!("{:#?}", report.translations()));
    let class_literal_lines = main
        .rust()
        .lines()
        .filter(|line| line.contains("JavaClass"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        main.rust()
            .contains("JavaClass::<AItem>::literal(\"a.Item\")"),
        "missing a.Item class literal:\n{class_literal_lines}"
    );
    assert!(
        main.rust()
            .contains("JavaClass::<BItem>::literal(\"b.Item\")"),
        "missing b.Item class literal:\n{class_literal_lines}"
    );
    assert!(
        !main.rust().contains("JavaClass::<Item>::literal(\"Item\")"),
        "class literal collapsed to simple Item"
    );
    assert!(
        a_item.rust().contains("pub struct AItem"),
        "{}",
        a_item.rust()
    );
    assert!(
        b_item.rust().contains("pub struct BItem"),
        "{}",
        b_item.rust()
    );
}

#[test]
fn same_package_simple_class_literal_keeps_java_package_name() {
    let translation = new_java2rust::translate_source(
        r#"
package a;

public class Item {
    public static String run() {
        return Item.class.getName();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("JavaClass::<Item>::literal(\"a.Item\")"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("JavaClass::<Item>::literal(\"Item\")"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "same_package_simple_class_literal",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn external_pack_object_facts_do_not_erase_primary_source_type_emission() {
    let primary_sources = vec![new_java2rust::BatchSource::new(
        "modeled/ModeledPrimary.java".to_string(),
        r#"
package modeled;

public class ModeledPrimary {
    private int value;

    public int getValue() {
        return value;
    }
}
"#
        .to_string(),
    )];
    let facts = new_java2rust::external_packs::parse_external_pack_manifest(
        r#"
        external-pack modeled modeled external-fact-model semantic
        external-pack-object modeled modeled.ModeledPrimary erased-object
        external-pack-object modeled ModeledPrimary erased-object
        "#,
    )
    .facts()
    .clone();

    let report =
        new_java2rust::translate_batch_sources_with_context_sources_summary_sources_and_options(
            &primary_sources,
            &[],
            &[],
            true,
            &facts,
            &new_java2rust::BatchTranslationOptions::default(),
        );

    assert_eq!(report.translations().len(), 1);
    let rust = report.translations()[0].rust();
    assert!(rust.contains("pub struct ModeledPrimary"), "{rust}");
    assert!(rust.contains("pub fn get_value(&self) -> i32"), "{rust}");
    assert!(
        !rust.contains("pub type ModeledPrimary = Box<dyn std::any::Any>;"),
        "{rust}"
    );
}

#[test]
fn count_invalid_patterns_cli_uses_registry_counter() {
    let dir = unique_temp_dir("count_invalid_patterns_cli");
    let rust_path = dir.join("generated.rs");
    fs::write(
        &rust_path,
        r#"
pub fn bad() {
    compile_error!("unsupported Java expression: lambda");
    let literal = "compile_error!";
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("count-invalid-patterns")
        .arg(&rust_path)
        .output()
        .expect("run count-invalid-patterns");
    assert!(
        output.status.success(),
        "count-invalid-patterns failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
}

#[test]
fn translate_batch_cli_writes_phase_and_worker_events() {
    let dir = unique_temp_dir("translate_batch_events");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    let phase_events = out_dir.join("phase-events.tsv");
    let worker_events = out_dir.join("worker-events.tsv");
    write_batch_sources(&source_dir);
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(&phase_events, "phase\tstatus\telapsed_ms\tdetail\n").unwrap();
    fs::write(
        &worker_events,
        "event\tpid\tindex\tsource\telapsed_ms\tstatus\trust_path\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--phase-events")
        .arg(&phase_events)
        .arg("--worker-events")
        .arg(&worker_events)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    assert_eq!(status.lines().count(), 3, "{status}");
    assert!(
        status.starts_with(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\n"
        ),
        "{status}"
    );
    assert!(
        status.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7
        }),
        "{status}"
    );
    assert!(status.contains("\tok\t"), "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    assert_eq!(manifest.lines().count(), 3, "{manifest}");
    assert!(
        manifest.lines().skip(1).all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            fields.len() == 7 && fields[3].len() == 16
        }),
        "{manifest}"
    );

    let phase_events = fs::read_to_string(phase_events).unwrap();
    assert!(
        phase_events.contains("batch_primary_parse\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_analysis\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_resolve_context\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_semantic\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_representation\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_global_signature_fixpoint\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("batch_emission\tok\t"),
        "{phase_events}"
    );
    assert!(
        phase_events.contains("\tjobs=10"),
        "translate-batch should default to 10 jobs\n{phase_events}"
    );

    let analysis_profile = fs::read_to_string(out_dir.join("analysis-profile.tsv")).unwrap();
    assert!(
        analysis_profile.contains("metric\tvalue\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("primary_files\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("program_types\t2\n"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_slot_count\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_fixpoint_iterations\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_component_sweeps\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_state_clones\t"),
        "{analysis_profile}"
    );
    assert!(
        analysis_profile.contains("signature_propagation_arm_call_site_sweeps\t"),
        "{analysis_profile}"
    );

    let worker_events = fs::read_to_string(worker_events).unwrap();
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_global_signature\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events.lines().any(|line| {
            line.starts_with("batch_global_signature_fixpoint_iteration_live\t")
                && line.contains("\t0\t")
        }),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("batch_emission\t") && line.contains("\t0\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_start\t") && line.contains("\t1\t")),
        "{worker_events}"
    );
    assert!(
        worker_events
            .lines()
            .any(|line| line.starts_with("emit_end\t") && line.contains("\t2\t")),
        "{worker_events}"
    );
}

#[test]
fn translate_cli_exits_nonzero_on_java_syntax_errors() {
    let dir = unique_temp_dir("translate_cli_syntax_error");
    let source = dir.join("Broken.java");
    fs::write(
        &source,
        r#"
public class Broken {
    int bad(int a, int b) {
        return a + b ~;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg(&source)
        .output()
        .expect("run translator");
    assert!(
        !output.status.success(),
        "translator unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("java_syntax"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn translate_batch_status_reports_error_diagnostics() {
    let dir = unique_temp_dir("translate_batch_syntax_error_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("Broken.java"),
        r#"
public class Broken {
    int bad(int a, int b) {
        return a + b ~;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert_eq!(fields[2], "errors", "{status}");
    assert!(fields[3].parse::<usize>().unwrap_or(0) > 0, "{status}");
}

#[test]
fn translate_batch_cli_counts_unsupported_member_diagnostics_in_status() {
    let dir = unique_temp_dir("translate_batch_unsupported_member_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("InstanceInit.java"),
        r#"
public class InstanceInit {
    {
        int value = 1;
    }
}
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert!(fields[5].parse::<usize>().unwrap_or(0) > 0, "{status}");
}

#[test]
fn translate_batch_cli_counts_unmodelled_external_constants_in_status() {
    let dir = unique_temp_dir("translate_batch_stubbed_constant_status");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("UseExternalLimit.java"),
        r#"
public class UseExternalLimit {
    int limit() {
        return ExternalLimits.MAX_COUNT;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack external external semantic\n\
         external-pack-field external ExternalLimits MAX_COUNT semantic type=int\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .arg("--external-pack-manifest")
        .arg(&manifest)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let row = status
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing status row\n{status}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{status}");
    assert_eq!(fields[6], "1", "{status}");

    let manifest = fs::read_to_string(out_dir.join("batch-manifest.tsv")).unwrap();
    let row = manifest
        .lines()
        .nth(1)
        .unwrap_or_else(|| panic!("missing manifest row\n{manifest}"));
    let fields = row.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "{manifest}");
    assert_eq!(fields[6], "1", "{manifest}");
}

fn write_comparable_batch_output(root: &Path, hash: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("batch-manifest.tsv"),
        format!(
            "index\tsource\trust_path\trust_hash\tdiagnostics\tinvalid_patterns\tstubbed_constants\n1\tA.java\t{}/generated/00001_A.rs\t{}\t0\t0\t0\n",
            root.display(),
            hash
        ),
    )
    .unwrap();
    fs::write(
        root.join("status.tsv"),
        format!(
            "source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\tstubbed_constants\nA.java\t{}/generated/00001_A.rs\tok\t0\t0\t0\t0\n",
            root.display()
        ),
    )
    .unwrap();
    fs::write(
        root.join("invalid-patterns.tsv"),
        "pattern\tcount\tfiles\ncompile_error_placeholder\t0\t0\n",
    )
    .unwrap();
    fs::write(root.join("top-compile-errors.tsv"), "count\terror\n").unwrap();
    fs::write(
        root.join("summary.tsv"),
        "files\t1\ntranslate_failed_files\t0\ndiagnostic_files\t0\ninvalid_pattern_files\t0\ncargo_check\tok\ncargo_check_errors\t0\n",
    )
    .unwrap();
}

#[test]
fn focused_batch_artifact_comparator_ignores_output_prefixes_and_checks_hashes() {
    let dir = unique_temp_dir("batch_artifact_compare");
    let serial = dir.join("serial");
    let parallel = dir.join("parallel");
    write_comparable_batch_output(&serial, "0123456789abcdef");
    write_comparable_batch_output(&parallel, "0123456789abcdef");

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/compare-focused-batch-artifacts.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        output.status.success(),
        "comparator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    write_comparable_batch_output(&parallel, "fedcba9876543210");
    let output = Command::new("python3")
        .arg(script)
        .arg("--serial-dir")
        .arg(&serial)
        .arg("--parallel-dir")
        .arg(&parallel)
        .output()
        .expect("run comparator");
    assert!(
        !output.status.success(),
        "comparator should reject changed hash\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("batch-manifest differs"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn focused_package_offline_cargo_uses_local_ndarray_shim() {
    let dir = unique_temp_dir("focused_ndarray_shim");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("MatrixSource.java"),
        r#"
public class MatrixSource {
    public double[][] literal() {
        return new double[][] {{1.0, 2.0}, {3.0, 4.0}};
    }
}
"#,
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--offline-cargo")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cargo_toml = fs::read_to_string(out.join("generated-crate/Cargo.toml")).unwrap();
    assert!(
        cargo_toml.contains("ndarray = { path = \"ndarray-shim\" }"),
        "{cargo_toml}"
    );
    assert!(
        out.join("generated-crate/ndarray-shim/src/lib.rs").exists(),
        "missing local ndarray shim"
    );
    let ndarray_shim =
        fs::read_to_string(out.join("generated-crate/ndarray-shim/src/lib.rs")).unwrap();
    assert!(
        ndarray_shim.contains("pub fn from_rows(rows: &[&[T]]) -> Self")
            && ndarray_shim.contains("pub fn iter(&self) -> Rows<'_, T>"),
        "{ndarray_shim}"
    );
    let shared_support = fs::read_to_string(out.join("generated-shared-support.rs")).unwrap();
    assert!(
        shared_support.contains("fn java_normalize_scientific_string")
            && shared_support.contains("!(1e-3..1e7).contains(&magnitude)")
            && shared_support.contains("!(1e-3f32..1e7f32).contains(&magnitude)"),
        "assembled focused-package support must retain Java floating notation semantics:\n{shared_support}"
    );
    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tok"), "{summary}");
    assert!(summary.contains("cargo_failure_kind\tnone"), "{summary}");
    assert!(summary.contains("parse_failures\t0"), "{summary}");
    assert!(summary.contains("cargo_type_errors_valid\t1"), "{summary}");
    assert!(summary.contains("unsupported_statements\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t0"), "{summary}");
    assert!(summary.contains("unsupported_expressions\t0"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_types\t")
            .is_some_and(|count| count.parse::<usize>().is_ok())),
        "{summary}"
    );
    assert!(summary.contains("unmodelled_call_sites\t0"), "{summary}");
    assert!(summary.contains("noop_facade_methods\t"), "{summary}");
    assert!(summary.contains("noop_facade_call_sites\t"), "{summary}");
    assert!(
        summary.lines().any(|line| line
            .strip_prefix("unmodelled_upcast_conversions\t")
            .is_some_and(|count| count.parse::<usize>().is_ok_and(|count| count > 0))),
        "{summary}"
    );
    assert!(summary.contains("stubbed_constants\t0"), "{summary}");
    assert!(
        out.join("parse-failures.tsv").exists(),
        "missing parse failure report"
    );
}

#[cfg(unix)]
#[test]
fn focused_package_generated_rust_parse_error_invalidates_type_error_count() {
    let dir = unique_temp_dir("focused_parse_gate");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Broken.java"),
        r#"
public class Broken {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -u
set -o pipefail
if [[ "${1:-}" == "count-invalid-patterns" ]]; then
  printf '0\n'
  exit 0
fi
printf 'Warning unsupported_member: static initializer\n' >&2
printf 'pub struct Broken {\n'
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_translator).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--no-batch-translate")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail the gate when generated Rust does not parse\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary = fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert!(summary.contains("cargo_check\tfailed"), "{summary}");
    assert!(
        summary.contains("cargo_failure_kind\tgenerated_rust"),
        "{summary}"
    );
    assert!(summary.contains("cargo_type_errors_valid\t0"), "{summary}");
    assert!(summary.contains("unsupported_members\t1"), "{summary}");
    assert!(!summary.contains("parse_failures\t0"), "{summary}");
    let parse_failures = fs::read_to_string(out.join("parse-failures.tsv")).unwrap();
    assert!(
        parse_failures.contains("this file contains an unclosed delimiter"),
        "{parse_failures}"
    );
}

#[cfg(unix)]
#[test]
fn focused_package_missing_idiom_sweep_signal_fails_summary() {
    let dir = unique_temp_dir("focused_missing_idiom_signal");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Clean.java"),
        r#"
public class Clean {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -u
set -o pipefail
if [[ "${1:-}" == "count-invalid-patterns" ]]; then
  printf '0\n'
  exit 0
fi
printf 'pub struct Clean;\n'
"#,
    )
    .unwrap();
    let mut translator_permissions = fs::metadata(&fake_translator).unwrap().permissions();
    translator_permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, translator_permissions).unwrap();

    let real_python = Command::new("which")
        .arg("python3")
        .output()
        .expect("locate python3");
    assert!(
        real_python.status.success(),
        "python3 is required for focused wrapper test"
    );
    let real_python = String::from_utf8(real_python.stdout)
        .unwrap()
        .trim()
        .to_string();

    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_python = fake_bin.join("python3");
    fs::write(
        &fake_python,
        format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1:-}}" == *"/scripts/sweep-generated-idioms.py" ]]; then
  out_tsv=""
  out_md=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-tsv)
        out_tsv="$2"
        shift 2
        ;;
      --out-md)
        out_md="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  printf 'signal\tcount\tfiles\n' > "$out_tsv"
  printf 'unsupported_statements\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_members\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_expressions\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_types\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_methods\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_upcast_conversions\t0\t0\n' >> "$out_tsv"
  printf '# fake sweep\n' > "$out_md"
  exit 0
fi
exec "{}" "$@"
"#,
            real_python
        ),
    )
    .unwrap();
    let mut python_permissions = fs::metadata(&fake_python).unwrap().permissions();
    python_permissions.set_mode(0o755);
    fs::set_permissions(&fake_python, python_permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--no-batch-translate")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail when the idiom sweep omits a required signal\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing idiom signal: stubbed_constants"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn focused_package_missing_analysis_profile_metric_fails_even_with_idiom_fallback() {
    let dir = unique_temp_dir("focused_missing_analysis_profile_metric");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("Clean.java"),
        r#"
public class Clean {
    public int value() {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let fake_translator = dir.join("fake-translator.sh");
    fs::write(
        &fake_translator,
        r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "translate-batch" ]]; then
  out_dir=""
  generated_dir=""
  phase_events=""
  worker_events=""
  primary=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-dir)
        out_dir="$2"
        shift 2
        ;;
      --generated-dir)
        generated_dir="$2"
        shift 2
        ;;
      --phase-events)
        phase_events="$2"
        shift 2
        ;;
      --worker-events)
        worker_events="$2"
        shift 2
        ;;
      --primary)
        primary="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  mkdir -p "$generated_dir"
  printf 'source\trust_path\ttranslate_status\tdiagnostics\tinvalid_patterns\tunsupported_members\n' > "$out_dir/status.tsv"
  printf 'pub struct Clean;\n' > "$generated_dir/00001_Clean.rs"
  printf '%s/Clean.java\t%s/00001_Clean.rs\tok\t0\t0\t0\n' "$primary" "$generated_dir" >> "$out_dir/status.tsv"
  printf 'metric\tvalue\nprimary_files\t1\n' > "$out_dir/analysis-profile.tsv"
  printf 'batch_global_semantic\tok\t0\tfake\n' >> "$phase_events"
  printf 'batch_translate_end\t%s\t0\t%s\t0\tok\t%s\n' "$$" "$primary" "$generated_dir" >> "$worker_events"
  exit 0
fi
printf 'unexpected fake translator command: %s\n' "${1:-}" >&2
exit 1
"#,
    )
    .unwrap();
    let mut translator_permissions = fs::metadata(&fake_translator).unwrap().permissions();
    translator_permissions.set_mode(0o755);
    fs::set_permissions(&fake_translator, translator_permissions).unwrap();

    let real_python = Command::new("which")
        .arg("python3")
        .output()
        .expect("locate python3");
    assert!(
        real_python.status.success(),
        "python3 is required for focused wrapper test"
    );
    let real_python = String::from_utf8(real_python.stdout)
        .unwrap()
        .trim()
        .to_string();

    let fake_bin = dir.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_python = fake_bin.join("python3");
    fs::write(
        &fake_python,
        format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "${{1:-}}" == *"/scripts/sweep-generated-idioms.py" ]]; then
  out_tsv=""
  out_md=""
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --out-tsv)
        out_tsv="$2"
        shift 2
        ;;
      --out-md)
        out_md="$2"
        shift 2
        ;;
      *)
        shift
        ;;
    esac
  done
  printf 'signal\tcount\tfiles\n' > "$out_tsv"
  printf 'unsupported_statements\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_members\t0\t0\n' >> "$out_tsv"
  printf 'unsupported_expressions\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_types\t0\t0\n' >> "$out_tsv"
  printf 'unmodelled_call_sites\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_methods\t0\t0\n' >> "$out_tsv"
  printf 'noop_facade_call_sites\t7\t1\n' >> "$out_tsv"
  printf 'unmodelled_upcast_conversions\t0\t0\n' >> "$out_tsv"
  printf 'stubbed_constants\t0\t0\n' >> "$out_tsv"
  printf '# fake sweep\n' > "$out_md"
  exit 0
fi
exec "{}" "$@"
"#,
            real_python
        ),
    )
    .unwrap();
    let mut python_permissions = fs::metadata(&fake_python).unwrap().permissions();
    python_permissions.set_mode(0o755);
    fs::set_permissions(&fake_python, python_permissions).unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let output = Command::new(&script)
        .arg("--skip-resolver-bench")
        .arg("--jobs")
        .arg("1")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .env("NEW_JAVA2RUST_BIN", &fake_translator)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .expect("run focused package wrapper");
    assert!(
        !output.status.success(),
        "focused wrapper should fail when analysis profile exists but omits a required metric\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("missing analysis profile metric: noop_facade_call_sites"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn focused_package_wrapper_preserves_primary_source_types_covered_by_external_pack() {
    let dir = unique_temp_dir("focused_primary_pack_overlap");
    let primary = dir.join("java");
    let out = dir.join("out");
    fs::create_dir_all(&primary).unwrap();
    fs::write(
        primary.join("ModeledPrimary.java"),
        r#"
package modeled;

public class ModeledPrimary {
    public enum Kind {
        ONE,
        TWO
    }

    public int value() {
        return 1;
    }

    public ModeledPeer peer() {
        return new ModeledPeer();
    }
}
"#,
    )
    .unwrap();
    fs::write(
        primary.join("ModeledPeer.java"),
        r#"
package modeled;

public class ModeledPeer {
    public int two() {
        return 2;
    }

    public int kinds() {
        return ModeledPrimary.Kind.values().length;
    }
}
"#,
    )
    .unwrap();
    let manifest = dir.join("pack.txt");
    fs::write(
        &manifest,
        "external-pack modeled ModeledPrimary semantic\n\
         external-pack modeled ModeledPeer semantic\n\
         external-pack-object modeled ModeledPrimary erased-object\n\
         external-pack-object modeled ModeledPeer erased-object\n",
    )
    .unwrap();

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/translate-focused-package.sh");
    let mut command = Command::new(&script);
    command
        .arg("--skip-resolver-bench")
        .arg("--jobs")
        .arg("2")
        .arg("--primary")
        .arg(&primary)
        .arg("--out")
        .arg(&out)
        .arg("--external-pack-manifest")
        .arg(&manifest);
    if let Ok(binary) = std::env::var("CARGO_BIN_EXE_new-java2rust") {
        command.env("NEW_JAVA2RUST_BIN", binary);
    }
    let output = command.output().expect("run focused package wrapper");
    assert!(
        output.status.success(),
        "focused wrapper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut generated = String::new();
    for entry in fs::read_dir(out.join("generated-crate/src/generated")).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs") {
            generated.push_str(&fs::read_to_string(entry.path()).unwrap());
        }
    }
    assert!(
        generated.contains("pub struct ModeledPrimary"),
        "{generated}"
    );
    assert!(
        generated.contains("pub enum ModeledPrimary_Kind"),
        "{generated}"
    );
    assert!(
        generated.contains("ModeledPrimary_Kind::values()"),
        "{generated}"
    );
    assert!(
        !generated.contains("pub struct ModeledPrimaryKind;"),
        "{generated}"
    );
    assert!(generated.contains("pub struct ModeledPeer"), "{generated}");
}

/// One unparseable file in a batch must not mark its clean siblings `errors`.
///
/// Batch mode pooled every *primary* file's parse diagnostics into a batch-wide
/// bag and then appended that bag to each file's own diagnostics. Every file
/// therefore carried every other file's parse errors, so a single bad file made
/// the whole batch read `errors`, and the bad file was counted twice -- its own
/// diagnostics reach it directly through `parsed.diagnostics` as well. The
/// column had gone from constant-`ok` (the original 0055 finding) to
/// effectively constant-`errors`, which is still not a measurement.
///
/// The bag is for context and summary sources, which genuinely are shared by
/// every file; the single-file path always kept that invariant.
///
/// See claude_audits/0055-batch-translation-status-is-not-measured.md.
#[test]
fn translate_batch_keeps_parse_diagnostics_with_the_file_that_caused_them() {
    let dir = unique_temp_dir("translate_batch_diagnostic_isolation");
    let source_dir = dir.join("java");
    let out_dir = dir.join("out");
    let generated_dir = out_dir.join("generated");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&generated_dir).unwrap();
    fs::write(
        source_dir.join("CleanOne.java"),
        "public class CleanOne { public static int run() { return 7; } }\n",
    )
    .unwrap();
    fs::write(
        source_dir.join("CleanTwo.java"),
        "public class CleanTwo { public static int run() { return 8; } }\n",
    )
    .unwrap();
    fs::write(
        source_dir.join("Unparseable.java"),
        "public class Unparseable { public static int run() { return ((( } }\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_new-java2rust"))
        .arg("translate-batch")
        .arg("--primary")
        .arg(&source_dir)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--generated-dir")
        .arg(&generated_dir)
        .output()
        .expect("run translate-batch");
    assert!(
        output.status.success(),
        "translate-batch failed\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let status = fs::read_to_string(out_dir.join("status.tsv")).unwrap();
    let mut seen = 0;
    for line in status.lines().skip(1) {
        let fields = line.split('\t').collect::<Vec<_>>();
        let (source, translate_status, diagnostics) = (fields[0], fields[2], fields[3]);
        seen += 1;
        if source.ends_with("Unparseable.java") {
            assert_eq!(translate_status, "errors", "{status}");
            // Exactly once -- not once from its own parse and again from the bag.
            assert_eq!(diagnostics, "1", "{status}");
        } else {
            assert_eq!(
                translate_status, "ok",
                "a clean file inherited another file's parse errors\n{status}"
            );
            assert_eq!(diagnostics, "0", "{status}");
        }
    }
    assert_eq!(seen, 3, "{status}");
}

#[test]
fn batch_cross_file_overloaded_constructor_uses_the_emitted_fallibility_abi() {
    let record = r#"
package example.model;
public class Record {
    String value;
    public Record(String value) {
        this.value = value;
        if (value.length() == 0) throw new RuntimeException("empty");
    }
    public Record(Record base, int start, int length) {
        if (start < 0) throw new RuntimeException("negative");
        this.value = base.value.substring(start, start + length);
    }
    public Record(Record base, String value, String quality, int offset) {
        this.value = value;
    }
    public void rename(String value) { this.value = value; }
}
"#;
    let consumer = r#"
package example.consumer;
import example.model.Record;
public class Consumer {
    public Record convert(Record input, String value) {
        return new Record(input, value, value, 33);
    }
}
"#;
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "example/model/Record.java".to_string(),
                    record.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/consumer/Consumer.java".to_string(),
                    consumer.to_string(),
                ),
            ],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let consumer = translations
        .iter()
        .find(|translation| translation.path() == "example/consumer/Consumer.java")
        .expect("translated consumer");
    assert!(
        consumer.rust().contains(
            "Record::new_record_string_string_int(input, std::clone::Clone::clone(&value)"
        ),
        "{}",
        consumer.rust()
    );
    assert!(!consumer.rust().contains(", 33)?"), "{}", consumer.rust());
}

#[test]
fn batch_flattened_super_constructor_keeps_private_self_call_statically_bound() {
    let base = r#"
package example.model;
public class Base {
    int value;
    public Base(int value) {
        this.value = value;
        validate();
    }
    private void validate() {
        if (value < 0) throw new IllegalArgumentException("negative");
    }
}
"#;
    let child = r#"
package example.model;
public class Child extends Base {
    public Child(int value) { super(value); }
}
"#;
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "example/model/Base.java".to_string(),
                    base.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/model/Child.java".to_string(),
                    child.to_string(),
                ),
            ],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let child = translations
        .iter()
        .find(|translation| translation.path() == "example/model/Child.java")
        .expect("translated child")
        .rust();

    assert!(child.contains("fn __java_private_base_validate"), "{child}");
    assert!(
        child.contains("__java_self.__java_private_base_validate()")
            || child.contains("__java_private_base_validate_with_outer"),
        "flattened superclass construction must retain private static binding: {child}"
    );
    assert!(
        !child.contains("__java_self.validate()"),
        "a private superclass helper is not a virtual child call: {child}"
    );
}

#[test]
fn batch_nullable_constructor_receiver_uses_one_fallibility_abi_at_definition_and_call() {
    let record = r#"
package example.model;
public class Record {
    String name;
    String sequence;
    String quality;
    int offset;
    public Record(String name, String sequence, String quality, int offset) {
        this.name = name;
        this.sequence = sequence;
        this.quality = quality;
        if (sequence.length() != quality.length()) throw new RuntimeException("mismatch");
    }
    public Record(Record base, int start, int length) {
        if (start < 0) throw new RuntimeException("negative");
        this.sequence = base.sequence.substring(start, start + length);
    }
    public Record(Record base, String sequence, String quality, int offset) {
        this.name = base.name;
        this.sequence = sequence;
        this.quality = quality;
        this.offset = offset;
    }
    public String getSequence() { return sequence; }
    public String getQuality() { return quality; }
    public int getOffset() { return offset; }
}
"#;
    let base = r#"
package example.convert;
import example.model.Record;
public abstract class Base {
    public abstract Record convert(Record input);
}
"#;
    let consumer = r#"
package example.convert;
import example.model.Record;
public class Consumer extends Base {
    public Record convert(Record input) {
        if (input.getOffset() == 33) return input;
        String sequence = input.getSequence();
        String quality = input.getQuality();
        StringBuilder converted = new StringBuilder();
        for (int i = 0; i < quality.length(); i++) converted.append(quality.charAt(i));
        return new Record(input, sequence, converted.toString(), 33);
    }
}
"#;
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "example/model/Record.java".to_string(),
                    record.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/convert/Base.java".to_string(),
                    base.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/convert/Consumer.java".to_string(),
                    consumer.to_string(),
                ),
            ],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let consumer = translations
        .iter()
        .find(|translation| translation.path() == "example/convert/Consumer.java")
        .expect("translated consumer");
    let record_translation = translations
        .iter()
        .find(|translation| translation.path() == "example/model/Record.java")
        .expect("translated record");
    assert!(
        consumer
            .rust()
            .contains("Record::new_record_string_string_int(input, sequence"),
        "{}",
        consumer.rust()
    );
    assert!(consumer.rust().contains(", 33)?"), "{}", consumer.rust());
    assert!(
        record_translation
            .rust()
            .contains("pub fn new_record_string_string_int(")
            && record_translation
                .rust()
                .contains(") -> Result<Record, JavaException>")
            && record_translation.rust().contains("return Ok(Record"),
        "the solved constructor fallibility must control both its signature and body:\n{}",
        record_translation.rust()
    );
}

#[test]
fn batch_indexed_shared_reference_receiver_borrows_the_payload() {
    let record = r#"
package example.model;
public class Record {
    String name;
    public Record(String name) { this.name = name; }
    public String getName() { return name; }
    public void rename(String name) { this.name = name; }
    public static String aliasMutation(Record record) {
        Record alias = record;
        alias.rename("changed");
        return record.name;
    }
}
"#;
    let consumer = r#"
package example.consumer;
import example.model.Record;
public class Consumer {
    public String firstName(Record[] records) {
        return records[0].getName();
    }
}
"#;
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "example/model/Record.java".to_string(),
                    record.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/consumer/Consumer.java".to_string(),
                    consumer.to_string(),
                ),
            ],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let consumer = translations
        .iter()
        .find(|translation| translation.path() == "example/consumer/Consumer.java")
        .expect("translated consumer");
    assert!(
        consumer.rust().contains(".borrow().get_name()")
            || (consumer.rust().contains("Record::get_name_with_outer(")
                && consumer.rust().contains(
                    ".as_ref().ok_or_else(|| JavaException::new(\"NullPointerException\""
                )),
        "{}",
        consumer.rust()
    );
}

#[test]
fn batch_nested_enum_loop_keeps_lexical_element_type_and_payload_accessors() {
    let owner = r#"
package example.cli;
import java.util.Arrays;
class WrongArgumentsException extends Exception {
    WrongArgumentsException(String message) { super(message); }
}
public class CommandLineArguments {
    static enum Arguments {
        A("-a", ""), B("-b", "x", "y");
        final String argString;
        final String[] allowedValues;
        String value;
        boolean isSet;
        Arguments(String argString, String... allowedValues) {
            this.isSet = false;
            this.argString = argString;
            if (allowedValues.length == 1 && !allowedValues[0].equals(""))
                this.allowedValues = new String[] { "" };
            else
                this.allowedValues = allowedValues;
        }
        boolean hasAllowedValues() { return allowedValues.length != 0; }
        void set(String value) { this.value = value; }
        boolean getIsSet() { return isSet; }
        void setIsSet(boolean isSet) { this.isSet = isSet; }
        public String toString() { return argString; }
        String get() throws WrongArgumentsException {
            if (!isSet) throw new WrongArgumentsException("missing");
            return value;
        }
    }

    static void parse(String[] values) throws WrongArgumentsException {
        outer:
        for (int i = 0; i < values.length; i++) {
            for (Arguments arg : Arguments.values())
                if (arg.argString.equals(values[i])) {
                    if (arg.getIsSet()) throw new WrongArgumentsException("duplicate");
                    arg.setIsSet(true);
                    if (arg.hasAllowedValues()) {
                        String subArg = values[++i];
                        if (!arg.allowedValues[0].equals("") &&
                            !Arrays.asList(arg.allowedValues).contains(subArg))
                            throw new WrongArgumentsException("invalid");
                        arg.set(subArg);
                    }
                    continue outer;
                }
        }
    }

    static String selected() throws WrongArgumentsException { return Arguments.A.get(); }

    static Arguments parseAction(String[] values) {
        if (values.length == 0) return null;
        Arguments selected = null;
        for (Arguments action : Arguments.values())
            if (action.toString().equals(values[0])) selected = action;
        return selected;
    }
}
"#;
    let consumer = r#"
package example.cli;
import example.cli.CommandLineArguments.Arguments;
public class Consumer {
    static boolean uses(Arguments value) { return value == Arguments.A; }
}
"#;
    let cli = r#"
package example.cli;
public class Cli {
    public static void main(String[] args) throws WrongArgumentsException {
        CommandLineArguments.parse(args);
        CommandLineArguments.parseAction(args);
    }
}
"#;
    let nullable_strings = r#"
package example.cli;
class NullableStrings {
    static String[] values() { return new String[] { null }; }
}
"#;
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "example/cli/CommandLineArguments.java".to_string(),
                    owner.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/cli/Consumer.java".to_string(),
                    consumer.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/cli/Cli.java".to_string(),
                    cli.to_string(),
                ),
                new_java2rust::BatchSource::new(
                    "example/cli/NullableStrings.java".to_string(),
                    nullable_strings.to_string(),
                ),
            ],
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let owner = translations
        .iter()
        .find(|translation| translation.path() == "example/cli/CommandLineArguments.java")
        .expect("translated enum owner")
        .rust();
    assert!(owner.contains("arg.arg_string()"), "{owner}");
    assert!(
        (owner.contains("arg.set_is_set(true)") || owner.contains("arg.__java_set_is_set(true)"))
            && owner.contains("arg.has_allowed_values()")
            && owner.contains("arg.allowed_values()")
            && !owner.contains("Arg::"),
        "{owner}"
    );
    cargo_check_generated_modules(
        "batch_nested_enum_payload_accessors",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: owner.to_string(),
            tests: "",
        }],
    );
}

#[test]
fn batch_constructor_argument_preserves_closed_hierarchy_carrier() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/model/Observation.java".to_string(),
            "package example.model; public abstract class Observation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/model/Model.java".to_string(),
            r#"
package example.model;
public class Model<O extends Observation> {
    private int count;
    public O value() { return null; }
    public void bump() { count++; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/model/SpecializedModel.java".to_string(),
            r#"
package example.model;
public class SpecializedModel<O extends Observation> extends Model<O> {
    public O value() { return null; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/consume/Consumer.java".to_string(),
            r#"
package example.consume;
import example.model.Model;
import example.model.Observation;
public class Consumer<O extends Observation> {
    private final Model<O> model;
    public Consumer(Model<O> model) {
        if (model == null) throw new IllegalArgumentException();
        this.model = model;
    }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/use/Caller.java".to_string(),
            r#"
package example.use;
import example.consume.Consumer;
import example.model.Model;
import example.model.Observation;
import example.model.SpecializedModel;
public class Caller {
    public static <O extends Observation> Consumer<O> make(Model<O> model) {
        model.bump();
        return new Consumer<O>(model);
    }
    public static <O extends Observation> Model<O> specialized() {
        return new SpecializedModel<O>();
    }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let caller = translations
        .iter()
        .find(|translation| translation.path() == "example/use/Caller.java")
        .expect("translated caller");
    assert!(
        caller.rust().contains("Consumer::new(Some(model))")
            && !caller.rust().contains("match model { ModelValue::"),
        "{}",
        caller.rust()
    );
}

#[test]
fn batch_overloaded_source_call_uses_selected_mixed_parameter_abis() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/model/Observation.java".to_string(),
            "package example.model; public abstract class Observation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/io/Token.java".to_string(),
            r#"
package example.io;
public class Token {
    int position;
    public void advance() { position++; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/io/ObservationReader.java".to_string(),
            r#"
package example.io;
import example.model.Observation;
public abstract class ObservationReader<O extends Observation> {
    public abstract O read(Token token);
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/io/ConcreteReader.java".to_string(),
            r#"
package example.io;
import example.model.Observation;
public class ConcreteReader<O extends Observation> extends ObservationReader<O> {
    public O read(Token token) { token.advance(); return null; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/io/SequenceReader.java".to_string(),
            r#"
package example.io;
import example.model.Observation;
public class SequenceReader {
    public static <O extends Observation> ObservationReader<O> concrete() {
        return new ConcreteReader<O>();
    }
    public static <O extends Observation> O read(ObservationReader<O> reader, String input) {
        Token token = new Token();
        O value = null;
        while (token.position < 2) value = read(reader, token);
        return value;
    }
    public static <O extends Observation> O read(ObservationReader<O> reader, Token token) {
        token.advance();
        while (token.position < 2) reader.read(token);
        return reader.read(token);
    }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let sequence_reader = translations
        .iter()
        .find(|translation| translation.path() == "example/io/SequenceReader.java")
        .expect("translated sequence reader");
    assert!(
        sequence_reader
            .rust()
            .contains("reader: ObservationReaderValue<O>, token: &mut Token")
            && sequence_reader
                .rust()
                .contains("read_observation_reader_token(std::clone::Clone::clone(&reader)")
            && sequence_reader.rust().contains(", &mut token)?"),
        "{}",
        sequence_reader.rust()
    );
}

#[test]
fn batch_specialized_generic_subclass_keeps_direct_reader_abi() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/Observation.java".to_string(),
            "package example; public abstract class Observation {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/Token.java".to_string(),
            "package example; public class Token { public void advance() {} }".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/ObservationReader.java".to_string(),
            r#"
package example;
public abstract class ObservationReader<O extends Observation> {
    public abstract O read(Token token);
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/IntegerObservation.java".to_string(),
            "package example; public final class IntegerObservation extends Observation {}"
                .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/IntegerReader.java".to_string(),
            r#"
package example;
public final class IntegerReader extends ObservationReader<IntegerObservation> {
    public IntegerObservation read(Token token) { token.advance(); return null; }
}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/SequenceReader.java".to_string(),
            r#"
package example;
public class SequenceReader {
    public static <O extends Observation> O read(ObservationReader<O> reader, String input) {
        Token token = new Token();
        return read(reader, token);
    }
    public static <O extends Observation> O read(ObservationReader<O> reader, Token token) {
        return reader.read(token);
    }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let sequence_reader = translations
        .iter()
        .find(|translation| translation.path() == "example/SequenceReader.java")
        .expect("translated sequence reader");
    assert!(
        sequence_reader
            .rust()
            .contains("reader: ObservationReader<O>, token: &mut Token"),
        "{}",
        sequence_reader.rust()
    );
    assert!(
        sequence_reader
            .rust()
            .contains("read_observation_reader_token(reader, &mut token)?"),
        "{}",
        sequence_reader.rust()
    );
}

#[test]
fn batch_concrete_inherited_value_reuses_mutable_borrow_abi() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/Base.java".to_string(),
            "package example; public class Base { int value; public void bump() { value++; } }"
                .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/Child.java".to_string(),
            "package example; public class Child extends Base {}".to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/Use.java".to_string(),
            r#"
package example;
public class Use {
    private void first(Base value) { value.bump(); }
    private void second(Base value) { value.bump(); }
    private void third(Base value) { value.bump(); }
    public void run() {
        Base value = new Base();
        first(value);
        second(value);
        third(value);
    }
}

"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let use_translation = translations
        .iter()
        .find(|translation| translation.path() == "example/Use.java")
        .expect("translated use class");
    for method in ["first", "second", "third"] {
        assert!(
            use_translation
                .rust()
                .contains(&format!("Self::{method}(&mut value)?")),
            "{}",
            use_translation.rust()
        );
    }
}

#[test]
fn batch_inherited_super_call_uses_solved_result_abi() {
    let primary_sources = vec![
        new_java2rust::BatchSource::new(
            "example/BaseLine.java".to_string(),
            r#"
package example;
public class BaseLine {
    String value;
    public String render() {
        if (value == null) throw new IllegalStateException("missing");
        return value.substring(1);
    }
    public String toString() { return render(); }
    public boolean equals(Object other) {
        if (!(other instanceof BaseLine)) return false;
        BaseLine that = (BaseLine) other;
        return render().equals(that.render());
    }
}

"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/ChildLine.java".to_string(),
            r#"
package example;
public class ChildLine extends BaseLine {
    public boolean equals(Object other) {
        return !super.equals(other);
    }
    public String compareText(Object other) {
        return super.render() + other.toString();
    }
    public int compareTo(Object other) {
        return toString().compareTo(other.toString());
    }
}

"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/GrandChildLine.java".to_string(),
            r#"
package example;
public class GrandChildLine extends ChildLine {}
"#
            .to_string(),
        ),
        new_java2rust::BatchSource::new(
            "example/LineUse.java".to_string(),
            r#"
package example;
public class LineUse {
    public static boolean compareNull(BaseLine line) { return line.equals(null); }
    public static String show(BaseLine line) { return "line=" + line; }
    public static String showChild(ChildLine line) { return "child=" + line; }
}
"#
            .to_string(),
        ),
    ];
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &primary_sources,
            &[],
            &[],
            true,
            &new_java2rust::facts::FactSet::default(),
        );
    let child = translations
        .iter()
        .find(|translation| translation.path() == "example/GrandChildLine.java")
        .expect("translated grandchild");
    assert!(
        child.rust().contains("BaseLine::equals")
            && child.rust().contains(", other)?)")
            && !child
                .rust()
                .contains("java_string_compare_to(&self.to_string(),"),
        "inherited source calls must propagate the solved Result ABI"
    );
    let line_use = translations
        .iter()
        .find(|translation| translation.path() == "example/LineUse.java")
        .expect("translated line use");
    assert!(
        line_use.rust().contains("(line.to_string()?)")
            || line_use.rust().contains("line.to_string()?")
            || line_use
                .rust()
                .contains("line.to_string().unwrap_or_else(|ex|"),
        "source values in Java concatenation must discharge the exact fallible toString ABI: {}",
        line_use.rust()
    );
    let show_child = line_use
        .rust()
        .split("pub fn show_child")
        .nth(1)
        .and_then(|tail| tail.split("\n    }").next())
        .expect("rendered showChild body");
    assert!(
        show_child.contains("line.to_string().unwrap_or_else(|ex|"),
        "an inherited fallible toString must retain its Result ABI in Java concatenation: {}",
        line_use.rust()
    );
}
