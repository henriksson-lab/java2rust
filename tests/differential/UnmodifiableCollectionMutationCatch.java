import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

public class UnmodifiableCollectionMutationCatch {
    public static String run() {
        List<String> directValues = new ArrayList<>();
        directValues.add("a");

        String direct = "unset";
        try {
            Collections.unmodifiableList(directValues).add("b");
            direct = "miss";
        } catch (UnsupportedOperationException ex) {
            direct = "direct";
        }

        List<String> values = new ArrayList<>();
        values.add("a");

        Collection<String> collectionValues = new ArrayList<>();
        collectionValues.add("c");
        String collectionRemove = "unset";
        Collection<String> collectionView = Collections.unmodifiableCollection(collectionValues);
        try {
            collectionView.remove("c");
            collectionRemove = "miss";
        } catch (UnsupportedOperationException ex) {
            collectionRemove = "collectionRemove";
        }

        String alias = "unset";
        List<String> view = Collections.unmodifiableList(values);
        try {
            view.add("c");
            alias = "miss";
        } catch (UnsupportedOperationException ex) {
            alias = "alias";
        }

        String aliasClear = "unset";
        try {
            view.clear();
            aliasClear = "miss";
        } catch (UnsupportedOperationException ex) {
            aliasClear = "aliasClear";
        }

        Set<String> setValues = new HashSet<>();
        setValues.add("s");
        Set<String> setView = Collections.unmodifiableSet(setValues);
        String setRemove = "unset";
        try {
            setView.remove("s");
            setRemove = "miss";
        } catch (UnsupportedOperationException ex) {
            setRemove = "setRemove";
        }
        String setClear = "unset";
        try {
            Collections.unmodifiableSet(setValues).clear();
            setClear = "miss";
        } catch (UnsupportedOperationException ex) {
            setClear = "setClear";
        }

        Map<String, String> mapValues = new HashMap<>();
        mapValues.put("k", "v");
        Map<String, String> mapView = Collections.unmodifiableMap(mapValues);
        String mapPut = "unset";
        try {
            mapView.put("x", "y");
            mapPut = "miss";
        } catch (UnsupportedOperationException ex) {
            mapPut = "mapPut";
        }
        String mapClear = "unset";
        try {
            mapView.clear();
            mapClear = "miss";
        } catch (UnsupportedOperationException ex) {
            mapClear = "mapClear";
        }

        String emptyList = "unset";
        List<String> empty = Collections.emptyList();
        try {
            empty.add("e");
            emptyList = "miss";
        } catch (UnsupportedOperationException ex) {
            emptyList = "emptyList";
        }

        String emptySet = "unset";
        Set<String> noNames = Collections.emptySet();
        try {
            noNames.add("n");
            emptySet = "miss";
        } catch (UnsupportedOperationException ex) {
            emptySet = "emptySet";
        }

        String singleton = "unset";
        try {
            Collections.singletonList("one").set(0, "two");
            singleton = "miss";
        } catch (UnsupportedOperationException ex) {
            singleton = "singleton";
        }

        String copies = "unset";
        try {
            Collections.nCopies(2, "copy").remove("copy");
            copies = "miss";
        } catch (UnsupportedOperationException ex) {
            copies = "copies";
        }

        String fixed = "unset";
        List<String> fixedSize = Arrays.asList("x", "y");
        try {
            fixedSize.add("z");
            fixed = "miss";
        } catch (UnsupportedOperationException ex) {
            fixed = "fixed";
        }

        String old = fixedSize.set(1, "Y");

        String clear = "unset";
        try {
            fixedSize.clear();
            clear = "miss";
        } catch (UnsupportedOperationException ex) {
            clear = "clear";
        }

        return direct + "," + collectionRemove + "," + alias + "," + aliasClear + "," + setRemove + "," + setClear + "," + mapPut + "," + mapClear + "," + emptyList + "," + emptySet + "," + singleton + "," + copies + "," + fixed + "," + clear + ":" + old + ":" + fixedSize.get(1);
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
