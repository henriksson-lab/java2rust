package nullablegenericreceiver;
public class NullableGenericReceiverAudit {
    interface Reader { int get(); }
    interface Updater { int add(int delta); }
    interface Merge<T> { void merge(T other); }
    static class Counter implements Reader, Updater, Merge<Counter> {
        int value;
        Counter(int value) { this.value=value; }
        public int get() { return value; }
        public int add(int delta) {
            if(delta<0) throw new IllegalArgumentException("negative");
            value+=delta; return value;
        }
        public void merge(Counter other) { value+=other.value; }
    }
    static class SingleCounter implements Reader, Updater {
        int value;
        SingleCounter(int value) { this.value=value; }
        public int get() { return value; }
        public int add(int delta) { value+=delta; return value; }
    }
    static int effects;
    static int effect(boolean fail) {
        effects++;
        if(fail) throw new IllegalStateException("argument");
        return 4;
    }
    static <T extends Reader & Merge<T>> void mergeTwice(T value,T other) {
        if(value!=null) { value.merge(other); value.merge(other); }
    }
    static <T extends Reader & Updater> int bumpTwice(T value) {
        if(value==null) return -1;
        value.add(1);
        int first=value.add(2);
        return first+value.get();
    }
    static <T extends Reader & Updater> int invoke(T value,boolean fail) {
        return value.add(effect(fail));
    }
    static <T extends Updater> int single(T value,boolean fail) {
        return value.add(effect(fail));
    }
    static <T extends Updater> int negative(T value) { return value.add(-1); }
    public static int run() {
        Counter value=new Counter(10); Counter alias=value; Counter other=new Counter(3);
        mergeTwice(value,other);
        if(alias.get()!=16 || value!=alias || other.get()!=3) return 1;
        if(bumpTwice(value)!=38 || alias.get()!=19) return 2;
        SingleCounter unique=new SingleCounter(2);
        if(bumpTwice(unique)!=10 || unique.get()!=5) return 3;
        mergeTwice((Counter)null,other);
        if(bumpTwice((Counter)null)!=-1 || other.get()!=3) return 4;
        effects=0;
        if(invoke(value,false)!=23 || effects!=1 || alias.get()!=23) return 5;
        effects=0;
        try { invoke((Counter)null,false); return 6; }
        catch(NullPointerException expected) { if(effects!=1) return 7; }
        effects=0;
        try { invoke((Counter)null,true); return 8; }
        catch(IllegalStateException expected) { if(effects!=1) return 9; }
        effects=0;
        try { single((Counter)null,false); return 10; }
        catch(NullPointerException expected) { if(effects!=1) return 11; }
        effects=0;
        try { single((Counter)null,true); return 12; }
        catch(IllegalStateException expected) { if(effects!=1) return 13; }
        try { negative(value); return 14; }
        catch(IllegalArgumentException expected) { if(alias.get()!=23) return 15; }
        if(single(value,false)!=27 || alias.get()!=27) return 16;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
