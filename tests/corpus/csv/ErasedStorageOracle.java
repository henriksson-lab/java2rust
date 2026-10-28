public class ErasedStorageOracle {
    public static void main(String[] args) {
        String[] values = {"before", null};
        Object local = ErasedStorageContracts.erase(values);
        Object direct = ErasedStorageContracts.direct(values);
        ErasedStorageContracts owner = new ErasedStorageContracts(values);
        int[] ints = {1, 2};
        Object primitive = ErasedStorageContracts.primitive(ints);
        ints[0] = 7;
        ErasedStorageContracts.initializeShared();
        Object synchronizedValue = ErasedStorageContracts.synchronizedValue();
        ErasedStorageContracts.copyShared();
        String[] array = ErasedStorageContracts.nullStrings();
        Object[] view = (Object[]) ErasedStorageContracts.direct(array);
        view[0] = "changed";
        try { view[0] = Integer.valueOf(3); System.out.println(3); return; }
        catch (ArrayStoreException expected) { }
        try { view[-1] = Integer.valueOf(3); System.out.println(4); return; }
        catch (ArrayIndexOutOfBoundsException expected) { }
        if (!"changed".equals(array[0]) || view.length != 3) { System.out.println(5); return; }
        view[0] = null;
        Object[] objects = (Object[]) ErasedStorageContracts.erasedObjects(array);
        if (objects[0] != array || objects[2] != null) { System.out.println(6); return; }
        objects[1] = Integer.valueOf(7);
        if (!Integer.valueOf(7).equals(objects[1])) { System.out.println(7); return; }
        try { ErasedStorageContracts.checked(true); System.out.println(2); return; }
        catch (IllegalStateException expected) { }
        values[0] = "after";
        if (local != values || direct != values || owner.value() != values
                || owner.typedValue() != values || owner.typedLocal() != values
                || ErasedStorageContracts.viaCall(values) != values || ErasedStorageContracts.directCall(values) != values
                || primitive != ints || ((int[]) primitive)[0] != 7
                || synchronizedValue != ErasedStorageContracts.synchronizedValue()
                || !"[Ljava.lang.String;".equals(synchronizedValue.getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedEmptyStrings().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedNullStrings().getClass().getName())
                || !"[[Ljava.lang.String;".equals(ErasedStorageContracts.erasedRows().getClass().getName())
                || ErasedStorageContracts.nullRows()[0] != null
                || !"[[Ljava.lang.String;".equals(ErasedStorageContracts.erasedNullRows().getClass().getName())
                || !"[C".equals(ErasedStorageContracts.erasedUnits().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedCloneStrings().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedCopyStrings().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedRangeStrings().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedGenericClone().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedGenericCopy().getClass().getName())
                || ErasedStorageContracts.erasedSharedCopy() == synchronizedValue
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedSharedCopy().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.erasedConditionalClone().getClass().getName())
                || !"[Ljava.lang.String;".equals(ErasedStorageContracts.checked(false).getClass().getName())
                || !"checked".equals(((String[]) ErasedStorageContracts.checked(false))[0])
                || !"after".equals(((String[]) local)[0]) || ErasedStorageContracts.absent() != null) {
            System.out.println(1);
        } else { System.out.println(0); }
    }
}
