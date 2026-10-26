package collectionforeach;
import java.util.Set;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.function.Consumer;
enum Item { A(2), B(5), C(11); final int code; Item(int code){this.code=code;} }
class Accumulator {
    int sum; int calls;
    void add(Item x){ calls++; if(x==null) throw new NullPointerException("item"); sum+=x.code; }
    void reject(Item x){ calls++; throw new IllegalArgumentException("reject"); }
}
class PlainBuilder {
    int total;
    void add(Item value){ total+=value.code; }
}
class Lookalike {
    void forEach(Consumer<Item> action){ action.accept(Item.B); action.accept(Item.B); }
}
public class CollectionForEachAudit {
    static int effects;
    static Set<Item> kept;
    static Accumulator target;
    static Set<Item> pair(){ Set<Item> s=new LinkedHashSet<>(); s.add(Item.A); s.add(Item.B); return s; }
    static Accumulator bound(){ effects++; return target; }
    static Consumer<Item> action(){ effects+=10; return target::add; }
    static Consumer<Item> rebind(){ effects+=100; kept=new HashSet<>(); return target::add; }
    static Consumer<Item> fail(){ effects+=1000; throw new IllegalStateException("argument"); }
    public static int run(){
        Set<Item> values=pair(); Accumulator a=new Accumulator(); target=a;
        values.forEach(a::add);
        Consumer<Item> saved=a::add; values.forEach(saved);
        if(a.sum!=14 || a.calls!=4) return 1;
        effects=0; values.forEach(bound()::add);
        if(effects!=1 || a.sum!=21 || a.calls!=6) return 2;
        Set<Item> empty=new HashSet<>();
        try { empty.forEach((Consumer<Item>)null); return 3; } catch(NullPointerException expected){}
        effects=0;
        try { ((Set<Item>)null).forEach(action()); return 4; } catch(NullPointerException expected){}
        if(effects!=10 || a.calls!=6) return 5;
        try { ((Set<Item>)null).forEach(fail()); return 6; } catch(IllegalStateException expected){}
        if(effects!=1010) return 7;
        target=null; effects=0;
        try { empty.forEach(bound()::add); return 8; } catch(NullPointerException expected){}
        if(effects!=1) return 9;
        target=a; kept=pair(); effects=0;
        kept.forEach(rebind());
        if(effects!=100 || a.sum!=28 || a.calls!=8 || !kept.isEmpty()) return 10;
        try { values.forEach(a::reject); return 11; } catch(IllegalArgumentException expected){}
        if(a.calls!=9 || a.sum!=28) return 12;
        values.forEach(saved);
        if(a.calls!=11 || a.sum!=35) return 13;
        new Lookalike().forEach(a::add);
        new Lookalike().forEach(x -> a.add(x));
        if(a.calls!=15 || a.sum!=55) return 14;
        Set<Item> nullable=new HashSet<>(); nullable.add(null);
        try { nullable.forEach(saved); return 15; } catch(NullPointerException expected){}
        if(a.calls!=16 || a.sum!=55) return 16;
        Set<Item> changed=pair();
        try { changed.forEach(x -> { a.add(x); changed.add(Item.C); }); return 17; }
        catch(java.util.ConcurrentModificationException expected){}
        if(changed.size()!=3 || a.calls!=17 || a.sum!=57) return 18;
        PlainBuilder plain=new PlainBuilder();
        values.forEach(plain::add);
        if(plain.total!=7) return 19;
        return 0;
    }
    public static void main(String[] args){ System.out.println(run()); }
}
