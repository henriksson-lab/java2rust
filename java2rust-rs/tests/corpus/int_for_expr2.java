 class A { void m() {final int from = 3;
    final int to = source.length + 14;
    final double[] dest = MathArrays.copyOfRange(source, from, to);

    Assert.assertEquals(dest.length, to - from);
    for (int i = from; i < source.length; i++) {
        Assert.assertEquals(source[i + 1], dest[i - from + 1], 0);
    } };  }  