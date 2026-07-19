import java.util.LinkedHashSet;
import java.util.Set;

class UnresolvedSetReturnAddAll {
    Set<String> names;

    void mergeDefaults() {
        names.addAll(defaultNames());
    }
}
