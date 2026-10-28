import java.util.Arrays;
import java.util.Iterator;
import java.util.List;

class StoredArrayViewHolder {
    Object[] values;
    StoredArrayViewHolder(Object[] values) { this.values = values; }
    Object[] get() { return values; }
    Object at(int index) { return values[index]; }
    void set(int index, Object value) { values[index] = value; }
    int length() { return values.length; }
}
class StoredArrayViewStager {
    String[] values;
    Object[] saved;
    StoredArrayViewStager(String[] values) { this.values = values; }
    String[] values() { return values; }
    void save(Object... values) { saved = values; }
    void stage() { save((Object[])this.values()); }
}
public class StoredArrayViewContracts {
    static int effects;
    static String[] shared = new String[] { "shared", null };
    static String[] sharedValues() { return shared; }
    static int index() { effects = effects * 10 + 1; return 0; }
    static Object item() { effects = effects * 10 + 2; return "item"; }
    static void write(Object[] values) { values[index()] = item(); }
    static <T> T keep(T value) { return value; }
    static int count(Object[] values) {
        int count = 0;
        for (Object value : values) if (value != null) count++;
        return count;
    }
    static int varargs(Object... values) { return count(values); }
    static Object[] pack(Object... values) { return values; }
    static int liveLoop(Object[] values) {
        int stage = 0;
        for (Object value : values) {
            if (stage == 0) values[1] = "during";
            else if (!value.equals("during")) return 1;
            stage++;
        }
        return stage == 2 ? 0 : 1;
    }
    static Object[] forward(Object[] values) { return values; }
    static Iterable<Object> iterable(Object[] values) { return Arrays.asList(values); }
    static Iterable<Object> viaList(Object[] values) {
        List<Object> list = Arrays.asList(values);
        return list;
    }
    static Object[] checked(Object value) { return (Object[])value; }
    static int run() {
        String[] backing = new String[] { "first", null };
        StoredArrayViewHolder holder = new StoredArrayViewHolder(backing);
        Object[] alias = forward(holder.get());
        if (alias != backing || holder.length() != 2) return 1;
        backing[0] = "late";
        if (holder.at(0) != backing[0] || holder.at(1) != null) return 2;
        holder.set(0, "set");
        if (!backing[0].equals("set")) return 3;
        try { holder.set(0, Integer.valueOf(4)); return 4; }
        catch (ArrayStoreException expected) { }
        Iterator<Object> cursor = iterable(alias).iterator();
        backing[0] = "iterator";
        if (cursor.next() != backing[0] || cursor.next() != null) return 5;
        Object erased = alias;
        Object[] recovered = checked(erased);
        recovered[1] = "recovered";
        if (!backing[1].equals("recovered")) return 6;
        Object[] fresh = new Object[] { "fresh", null };
        fresh[0] = "changed";
        if (!fresh[0].equals("changed")) return 7;
        Object[] generic = keep(alias);
        if (generic != alias || count(generic) != 2) return 8;
        if (varargs(backing) != 2 || varargs("packed", null) != 1) return 9;
        if (checked(null) != null) return 10;
        effects = 0;
        try { write(null); return 11; }
        catch (NullPointerException expected) { }
        if (effects != 12) return 12;
        try { count(null); return 13; }
        catch (NullPointerException expected) { }
        try { checked(new int[] { 1 }); return 14; }
        catch (ClassCastException expected) { }
        Object[] genericNull = keep((Object[])null);
        if (genericNull != null) return 15;
        if (!erased.getClass().getName().equals("[Ljava.lang.String;")) return 16;
        if (alias.getClass() != backing.getClass()) return 17;
        if (liveLoop(alias) != 0 || !backing[1].equals("during")) return 18;
        Iterable<Object> via = viaList(alias);
        backing[0] = "list-forward";
        if (via.iterator().next() != backing[0]) return 19;
        Object[] upcast = (Object[])sharedValues();
        if (upcast != shared || upcast.getClass() != shared.getClass()) return 20;
        upcast[0] = "upcast";
        if (!shared[0].equals("upcast")) return 21;
        if (pack((Object[])sharedValues()) != shared) return 22;
        Object scalar = "single";
        Object[] packed = pack(scalar);
        if (packed.length != 1 || packed[0] != scalar) return 23;
        if (!packed.getClass().getName().equals("[Ljava.lang.Object;")) return 24;
        String[] absent = null;
        Object[] nullUpcast = (Object[])absent;
        if (nullUpcast != null) return 25;
        StoredArrayViewStager stager = new StoredArrayViewStager(sharedValues());
        stager.stage();
        if (stager.saved != shared) return 26;
        stager.saved[0] = "staged";
        if (!shared[0].equals("staged")) return 27;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
