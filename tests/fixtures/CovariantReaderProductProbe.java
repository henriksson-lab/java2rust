abstract class ReaderObservation { abstract int number(); }
class FirstReaderObservation extends ReaderObservation {
    int number;
    FirstReaderObservation(int number) { this.number = number; }
    int number() { return number; }
}
class SecondReaderObservation extends ReaderObservation {
    int number;
    SecondReaderObservation(int number) { this.number = number; }
    int number() { return number; }
}
class FirstReaderProduct implements ReaderProduct<FirstReaderObservation> {
    int value;
    FirstReaderProduct(int value) { this.value = value; }
    public int value() { return value; }
    public void set(int value) { this.value = value; }
    public int score(FirstReaderObservation observation) { return value + observation.number(); }
}
class SecondReaderProduct implements ReaderProduct<SecondReaderObservation> {
    int value;
    SecondReaderProduct(int value) { this.value = value; }
    public int value() { return value; }
    public void set(int value) { this.value = value; }
    public int score(SecondReaderObservation observation) { return value + observation.number(); }
}
abstract class ProductReader<P extends ReaderProduct<?>> {
    abstract P read(int seed);
    abstract int reads();
}
class FirstProductReader extends ProductReader<FirstReaderProduct> {
    int count;
    FirstReaderProduct read(int seed) { count++; return new FirstReaderProduct(seed); }
    int reads() { return count; }
}
class SecondProductReader extends ProductReader<SecondReaderProduct> {
    int count;
    SecondReaderProduct read(int seed) { count++; return new SecondReaderProduct(seed); }
    int reads() { return count; }
}
class StatelessProductReader extends ProductReader<ReaderProduct<?>> {
    ReaderProduct<?> read(int seed) { return new FirstReaderProduct(seed); }
    int reads() { return 0; }
}
interface ProductReaderProvider {
    ProductReader<FirstReaderProduct> reader();
}
class ConcreteProductReaderProvider implements ProductReaderProvider {
    public FirstProductReader reader() { return new FirstProductReader(); }
}
abstract class ProductReaderProviderBase {
    abstract ProductReader<FirstReaderProduct> reader();
}
class ConcreteProductReaderProviderSubclass extends ProductReaderProviderBase {
    FirstProductReader reader() { return new FirstProductReader(); }
}
class CovariantReaderProductProbe {
    static int run() {
        ProductReader<ReaderProduct<?>> stateless = new StatelessProductReader();
        if (stateless.read(29).value() != 29 || stateless.reads() != 0) return 11;
        ProductReaderProvider provider = new ConcreteProductReaderProvider();
        if (provider.reader().read(19).value() != 19) return 9;
        ProductReaderProviderBase baseProvider = new ConcreteProductReaderProviderSubclass();
        if (baseProvider.reader().read(23).value() != 23) return 10;
        FirstProductReader source = new FirstProductReader();
        ProductReader<? extends ReaderProduct<?>> reader = source;
        ProductReader<? extends ReaderProduct<?>> alias = reader;
        ReaderProduct first = reader.read(7);
        ReaderProduct next = alias.read(8);
        if (first == next || source.reads() != 2 || first.value() != 7 || next.value() != 8) return 1;
        FirstReaderProduct concrete = (FirstReaderProduct) first;
        concrete.set(9);
        if (first.value() != 9 || !(first instanceof FirstReaderProduct) || first instanceof SecondReaderProduct) return 2;
        if (concrete.score(new FirstReaderObservation(4)) != 13) return 6;
        try { concrete.score(null); return 8; }
        catch (NullPointerException expected) { }
        FirstProductReader back = (FirstProductReader) reader;
        back.read(0);
        if (source.reads() != 3 || reader != source || !(reader instanceof FirstProductReader) || reader instanceof SecondProductReader) return 3;
        Object erased = reader;
        ProductReader<? extends ReaderProduct<?>> recovered = (ProductReader<? extends ReaderProduct<?>>) erased;
        ReaderProduct late = recovered.read(13);
        if (late.value() != 13 || source.reads() != 4 || recovered != reader) return 5;
        ProductReader<? extends ReaderProduct<?>> other = new SecondProductReader();
        ReaderProduct second = other.read(11);
        if (second.value() != 11 || !(second instanceof SecondReaderProduct) || other.reads() != 1) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
