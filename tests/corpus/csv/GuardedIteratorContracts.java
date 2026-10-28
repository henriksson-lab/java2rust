import java.util.Iterator;
import java.util.NoSuchElementException;

class GuardedIteratorItem {
    int value;
    GuardedIteratorItem(int value) { this.value = value; }
}
class GuardedIteratorOwner implements Iterable<GuardedIteratorItem> {
    GuardedIteratorItem pending;
    int reads;
    boolean closed;
    int construction;
    Cursor cursor = new Cursor();
    GuardedIteratorOwner(GuardedIteratorItem value) {
        if (construction != 1) throw new IllegalStateException("initializer order");
        pending = value;
        construction = 2;
    }
    GuardedIteratorItem read() {
        reads++;
        GuardedIteratorItem value = pending;
        pending = null;
        return value;
    }
    class Cursor implements Iterator<GuardedIteratorItem> {
        GuardedIteratorItem current;
        Cursor() {
            if (pending != null) throw new IllegalStateException("default reference state");
            construction = 1;
        }
        public boolean hasNext() {
            if (closed) return false;
            if (current == null) current = read();
            return current != null;
        }
        public GuardedIteratorItem next() {
            if (closed) throw new NoSuchElementException("closed");
            GuardedIteratorItem next = current;
            current = null;
            if (next == null) {
                next = read();
                if (next == null) throw new NoSuchElementException("exhausted");
            }
            return next;
        }
        public void remove() { throw new UnsupportedOperationException(); }
    }
    public Iterator<GuardedIteratorItem> iterator() { return cursor; }
}
public class GuardedIteratorContracts {
    static Iterator<GuardedIteratorItem> escaping(GuardedIteratorItem value) {
        return new GuardedIteratorOwner(value).iterator();
    }
    static int run() {
        GuardedIteratorItem item = new GuardedIteratorItem(17);
        GuardedIteratorOwner owner = new GuardedIteratorOwner(item);
        Iterator<GuardedIteratorItem> cursor = owner.iterator();
        if (owner.construction != 2) return 11;
        if (owner.reads != 0 || !cursor.hasNext() || !cursor.hasNext() || owner.reads != 1) return 1;
        item.value = 23;
        if (cursor.next() != item || item.value != 23 || owner.reads != 1) return 2;
        if (cursor.hasNext() || owner.reads != 2) return 3;
        try { cursor.next(); return 4; } catch (NoSuchElementException expected) {}
        try { cursor.remove(); return 5; } catch (UnsupportedOperationException expected) {}
        Iterator<GuardedIteratorItem> direct = escaping(item);
        if (direct.next() != item) return 6;
        try { direct.next(); return 7; } catch (NoSuchElementException expected) {}
        GuardedIteratorOwner closed = new GuardedIteratorOwner(item);
        Iterator<GuardedIteratorItem> closedCursor = closed.iterator();
        closed.closed = true;
        if (closedCursor.hasNext() || closed.reads != 0) return 8;
        try { closedCursor.next(); return 9; } catch (NoSuchElementException expected) {}
        if (closed.reads != 0) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
