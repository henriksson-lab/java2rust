package resultsum;
import java.util.function.Function;
public class SamePackageFactoryProbe {
    static Sum saved;
    public static int run() {
        inputsum.Sum input=new inputsum.Sum(13);
        Function<inputsum.Sum,Sum> factory=Sum::new;
        Sum first=factory.apply(input); saved=first;
        Sum second=factory.apply(input); input.add(5);
        return first!=second && first.source==input && saved.value()==18 && second.value()==18 ? 0 : 1;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
