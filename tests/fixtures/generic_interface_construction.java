import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

interface SnapshotView<T> {
    List<T> values();
}

class SnapshotStorage<T> implements SnapshotView<T> {
    private final List<T> values = new ArrayList<T>();
    public List<T> values() { return values; }
}

interface MembershipView<T> {
    boolean contains(T value);
}

interface EmptyView<T> {}
class EmptyStorage<T> implements EmptyView<T> {}

interface MixedView<A, B> { List<A> values(); }
class MixedStorage<A, B> implements MixedView<A, B> {
    private final List<A> values = new ArrayList<A>();
    public List<A> values() { return values; }
}

class SnapshotEnvelope<T> {
    private final SnapshotView<T> view;
    SnapshotEnvelope(SnapshotView<T> view) { this.view = view; }
}

class MembershipStorage<T> implements MembershipView<T> {
    private final Set<T> values = new HashSet<T>();
    public boolean contains(T value) { return values.contains(value); }
}

class GenericInterfaceConstruction {
    static <T> SnapshotView<T> snapshot() { return new SnapshotStorage<T>(); }
    static <T> MembershipView<T> membership() { return new MembershipStorage<T>(); }
    static <T> SnapshotStorage<T> concrete() { return new SnapshotStorage<T>(); }
    static <T> EmptyView<T> empty() { return new EmptyStorage<T>(); }
    static <A, B> MixedView<A, B> mixed() { return new MixedStorage<A, B>(); }
    static <T> SnapshotView<T> handle(SnapshotView<T> value) { return value; }
    static <T> SnapshotView<T> local() {
        SnapshotView<T> value = new SnapshotStorage<T>();
        return value;
    }
    static <T> SnapshotView<T> argument() { return handle(new SnapshotStorage<T>()); }
    static <T> SnapshotEnvelope<T> enveloped() {
        return new SnapshotEnvelope<T>(new SnapshotStorage<T>());
    }
    static <T> SnapshotView<T> assigned(SnapshotView<T> initial) {
        initial = new SnapshotStorage<T>();
        return initial;
    }
    static <T> SnapshotView<T> forwarded() { return snapshot(); }
    static <T> SnapshotView<T> cast() { return (SnapshotView<T>) new SnapshotStorage<T>(); }
    static <T> SnapshotView<T> conditional(boolean first) {
        return first ? new SnapshotStorage<T>() : new SnapshotStorage<T>();
    }
    static <T> SnapshotView<T> nested(boolean first) {
        if (first) { return new SnapshotStorage<T>(); }
        return new SnapshotStorage<T>();
    }
}

class OwnerInterfaceConstruction<T> {
    SnapshotView<T> snapshot() { return new SnapshotStorage<T>(); }
}
