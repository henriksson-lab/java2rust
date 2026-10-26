package interfaceprerequisites;
public class InterfaceViewPrerequisitesAudit {
    interface Parent<T> { T value(); }
    interface Reader<T> { T read(); }
    static class CellValue<T> implements Parent<T> {
        T current;
        CellValue(T value) { current=value; }
        public T value() { return current; }
        void set(T value) { current=value; }
    }
    static class TwinValue<T> {
        T first;
        T second;
        TwinValue(T value) { first=value; second=value; }
    }
    static <T> Reader<T> captured(Parent<T> source) { return source::value; }
    static boolean present(Parent<?> source) { return source!=null; }
    interface ReadView { int read(); }
    interface NativeView extends ReadView, java.util.function.DoubleConsumer {}
    static NativeView receiver() {
        return new NativeView() {
            int total;
            public int read() { return total; }
            public void accept(double incomingValue) { total+=(int)incomingValue; }
        };
    }
    public static int run() {
        CellValue<String> cell=new CellValue<String>("first");
        Parent<String> source=cell;
        if(!present(source)) return 1;
        Reader<String> reader=captured(source);
        if(!reader.read().equals("first")) return 2;
        cell.set("second"); source=null;
        if(!reader.read().equals("second")) return 3;
        Parent<String> missing=null;
        if(present(missing)) return 4;
        NativeView callback=receiver(); NativeView alias=callback;
        callback.accept(2.0);
        if(alias.read()!=2) return 5;
        alias.accept(3.0);
        if(callback.read()!=5) return 6;
        TwinValue<String> twin=new TwinValue<String>("shared");
        if(!twin.first.equals("shared") || !twin.second.equals("shared")) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
