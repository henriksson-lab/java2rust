import java.util.*;

class DequeItem {
    int value;
    DequeItem(int value) { this.value = value; }
}
class StoredDeque<T> {
    Deque<T> values = new ArrayDeque<>();
    Deque<T> alias() { return values; }
    void push(T value) { values.push(value); }
    T pop() { return values.pop(); }
    T peek() { return values.peek(); }
    void append(T value) { values.addLast(value); }
    T first() { return values.getFirst(); }
    T last() { return values.getLast(); }
    T removeLast() { return values.removeLast(); }
}
class StagedDequeOwner {
    Deque<DequeItem> values = new ArrayDeque<>();
    int calls;
    DequeItem argument() { calls++; values.clear(); return new DequeItem(7); }
    void staged() { values.push(argument()); }
}
class SyncDequeOwner {
    static Deque<DequeItem> values = new ArrayDeque<>();
    static int probe() {
        values.clear();
        values.addLast(new DequeItem(8));
        return values.removeFirst().value;
    }
}
class RetainedDequeContracts {
    // No callers: these contracts must not depend on downstream null checks.
    static <T> T unusedHead(Queue<T> values) { return values.peek(); }
    static <T> T unusedPoll(Queue<T> values) { return values.poll(); }
    static int run() {
        StoredDeque<DequeItem> box = new StoredDeque<>();
        Deque<DequeItem> alias = box.alias();
        Queue<DequeItem> queue = alias;
        Iterator<DequeItem> empty = alias.iterator();
        DequeItem first = new DequeItem(1);
        box.push(first);
        if (empty.hasNext()) return 1;
        box.append(new DequeItem(2));
        box.push(new DequeItem(3));
        if (box.first().value != 3 || box.last().value != 2) return 2;
        DequeItem popped = box.pop();
        popped.value = 4;
        if (popped.value != 4 || box.peek().value != 1) return 3;
        DequeItem peeked = box.peek();
        peeked.value = 5;
        if (first.value != 5 || queue.element().value != 5) return 4;
        if (box.removeLast().value != 2 || queue.remove().value != 5) return 5;
        if (box.peek() != null) return 18;
        try { int unused = box.peek().value; return 19; }
        catch (NullPointerException expected) { }
        if (queue.peek() != null || queue.poll() != null) return 6;
        try { queue.remove(); return 7; }
        catch (NoSuchElementException expected) { }
        try { box.last(); return 8; }
        catch (NoSuchElementException expected) { }
        try { box.push(null); return 9; }
        catch (NullPointerException expected) { }
        if (!alias.isEmpty()) return 10;
        box.push(first);
        Deque<DequeItem> copy = new ArrayDeque<>(alias);
        copy.peek().value = 6;
        alias.clear();
        if (copy.size() != 1 || first.value != 6 || !queue.isEmpty()) return 11;
        alias.addLast(first);
        Iterator<DequeItem> cursor = alias.iterator();
        alias.clear();
        try { cursor.next(); return 12; }
        catch (ConcurrentModificationException expected) { }
        Deque<DequeItem> linked = new LinkedList<>();
        Queue<DequeItem> linkedQueue = linked;
        linked.addFirst(null);
        if (linked.size() != 1 || linkedQueue.peek() != null) return 13;
        try { new ArrayDeque<>(linked); return 14; }
        catch (NullPointerException expected) { }
        if (linkedQueue.remove() != null || !linked.isEmpty()) return 15;
        StagedDequeOwner staged = new StagedDequeOwner();
        staged.values.addLast(first);
        staged.staged();
        if (staged.calls != 1 || staged.values.size() != 1 || staged.values.peek().value != 7) return 16;
        if (SyncDequeOwner.probe() != 8) return 17;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
