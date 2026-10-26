package anonymousancestorslots;
public class AnonymousAncestorSlotsAudit {
    interface Parent { int step(int delta); int read(); default int twice() { return read()*2; } }
    interface Child extends Parent { int tag(); }
    interface Value<T> { T get(); }
    interface Derived<T> extends Value<T> { int tag(); }
    static final class Stored<T> implements Value<T> {
        T value;
        Stored(T value) { this.value=value; }
        public T get() { return value; }
    }
    static Parent make(int seed) {
        return new Child() {
            private int value=seed;
            public int step(int delta) {
                if(delta<0) throw new IllegalArgumentException("delta");
                value+=delta; return value;
            }
            public int read() { return value; }
            public int tag() { return 100+value; }
            public int twice() { return value*3; }
        };
    }
    static Value<Integer> generic(int seed) {
        return new Derived<Integer>() {
            public Integer get() { return seed; }
            public int tag() { return 7; }
        };
    }
    static int invoke(Parent value) { return value.step(3); }
    public static int run() {
        Parent first=make(2);
        Parent alias=first;
        if(first!=alias || first.read()!=2 || first.twice()!=6) return 1;
        if(invoke(alias)!=5 || first.read()!=5) return 2;
        if(first.step(4)!=9 || alias.read()!=9 || alias.twice()!=27) return 3;
        Parent other=make(20);
        if(other==first || other.read()!=20) return 4;
        try { alias.step(-1); return 5; }
        catch(IllegalArgumentException expected) { if(first.read()!=9 || other.read()!=20) return 6; }
        Parent local=new Child() {
            public int step(int delta) { return 40+delta; }
            public int read() { return 40; }
            public int tag() { return 140; }
        };
        if(invoke(local)!=43 || local.read()!=40 || local.twice()!=80) return 7;
        if(invoke(new Child() {
            public int step(int delta) { return 50+delta; }
            public int read() { return 50; }
            public int tag() { return 150; }
        })!=53) return 8;
        Value<Integer> generic=generic(17);
        if(generic.get()!=17 || generic(19).get()!=19) return 9;
        Child child=new Child() {
            public int step(int delta) { return 60+delta; }
            public int read() { return 60; }
            public int tag() { return 160; }
        };
        Parent parent=child;
        if(child!=parent || parent.read()!=60 || child.tag()!=160) return 10;
        Value<Integer> genericAlias=generic;
        if(genericAlias!=generic || genericAlias.get()!=17) return 11;
        Stored<Integer> stored=new Stored<Integer>(13);
        Value<Integer> view=stored;
        Value<Integer> viewAlias=view;
        if(view!=stored || view!=viewAlias || view.get()!=13) return 12;
        stored.value=21;
        if(viewAlias.get()!=21) return 13;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
