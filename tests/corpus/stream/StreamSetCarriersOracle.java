import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Set;

public class StreamSetCarriersOracle {
    public static void main(String[] arguments) {
        List<String> values = Arrays.asList("a", "b", "a");
        for (Set<String> set : Arrays.asList(
                StreamSetCarriers.copied(values), StreamSetCarriers.filtered(values),
                StreamSetCarriers.mapped(values), StreamSetCarriers.checked(values, false))) {
            assert set.size() == 2;
            assert set.contains("a") && set.contains("b");
        }
        assert StreamSetCarriers.checked(Collections.emptyList(), true).isEmpty();
        boolean caught = false;
        try { StreamSetCarriers.checked(values, true); }
        catch (IllegalArgumentException expected) { caught = true; }
        assert caught;
        System.out.println("stream-set-carriers: ok");
    }
}
