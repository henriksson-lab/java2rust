package constructorreference;
import java.util.function.Function;
import java.util.function.BiFunction;
import java.util.function.Supplier;
class Seed {
    int value;
    Seed(int value) { this.value=value; }
    void add(int amount) { value+=amount; }
}
interface Maker { Product create(Seed seed,int offset); }
class Product {
    final Seed seed;
    final int offset;
    final int[] values;
    Product() { this.seed=new Seed(0); this.offset=1; this.values=new int[]{1}; }
    Product(Seed seed) { this(seed,2); }
    Product(Seed seed,int offset) {
        if(offset<0) throw new IllegalArgumentException("offset");
        this.seed=seed; this.offset=offset; this.values=new int[]{offset};
    }
    Product(int[] values) { this.seed=new Seed(values[0]); this.offset=3; this.values=values; }
    int total() { return seed.value+offset; }
}
public class ConstructorReferenceAudit {
    static Product escaped;
    public static int run() {
        Seed seed=new Seed(5);
        Function<Seed,Product> one=Product::new;
        Product first=one.apply(seed); escaped=first;
        Product second=one.apply(seed);
        if(first==second || first.seed!=seed || second.seed!=seed) return 1;
        seed.add(3); if(first.total()!=10 || escaped.total()!=10) return 2;
        BiFunction<Seed,Integer,Product> two=Product::new;
        Product third=two.apply(seed,4);
        if(third.seed!=seed || third.total()!=12) return 3;
        try { two.apply(seed,-1); return 4; } catch(IllegalArgumentException expected) {}
        if(two.apply(seed,6).total()!=14) return 5;
        Supplier<Product> zero=Product::new;
        if(zero.get().total()!=1 || zero.get()==zero.get()) return 6;
        Function<int[],Product> arrays=Product::new;
        int[] data={4}; Product fourth=arrays.apply(data); escaped=fourth;
        data[0]=8; if(fourth.values!=data || escaped.values[0]!=8) return 7;
        Maker maker=Product::new;
        try { maker.create(seed,-2); return 8; } catch(IllegalArgumentException expected) {}
        if(maker.create(seed,9).total()!=17) return 9;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
