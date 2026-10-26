import java.util.ArrayList;
import java.util.List;
interface CopyValue<O> { O get(); void set(O value); }
class CopyCell implements CopyValue<String> {
    String value;
    CopyCell(String value) { this.value = value; }
    public String get() { return value; }
    public void set(String value) { this.value = value; }
}
class CopyHolder<O> {
    List<CopyValue<O>> values;
    CopyHolder(List<CopyValue<O>> initial) { values = new ArrayList<CopyValue<O>>(initial); }
    void addNull() { values.add(null); }
    CopyValue<O> get(int index) { return values.get(index); }
}
class InterfaceListCopyProbe {
    static int run() {
        CopyCell cell = new CopyCell("first");
        List<CopyValue<String>> initial = new ArrayList<CopyValue<String>>();
        initial.add(cell);
        CopyHolder<String> holder = new CopyHolder<String>(initial);
        initial.clear();
        CopyValue<String> copied = holder.get(0);
        if (copied != cell) return 1;
        copied.set("second");
        if (!cell.get().equals("second")) return 2;
        holder.addNull();
        if (holder.get(1) != null || holder.get(0) != cell) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
