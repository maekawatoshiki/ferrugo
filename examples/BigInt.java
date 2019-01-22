class BigIntegerList {
  public int n;
  public BigIntegerList next = null;
  public BigIntegerList(int n) {
    this.n = n;
  }
  public String asString() {
    return (this.next != null ? this.next.asString() : "") + this.n + " ";
  }
}

class BigInteger {
  public boolean sign; // +: true, -: false
  public BigIntegerList list;
  public BigInteger(int n) {
    this.sign = n >= 0;
    this.list = new BigIntegerList(n);
  }
  public String asString() {
    return (this.sign ? "" : "-") + this.list.asString();
  }
  public BigInteger add(BigInteger val) {
    int carry = 0;
    BigInteger c = new BigInteger(0);
    BigIntegerList al = this.list;
    BigIntegerList bl = val.list;
    BigIntegerList cl = c.list;

    for (;;) {
      cl.n += carry;
      if (al != null) {
        cl.n += al.n;
        al = al.next;
      }
      if (bl != null) {
        cl.n += bl.n;
        bl = bl.next;
      }
      carry = cl.n / 1000000000;
      cl.n %=        1000000000;

      if (al != null || bl != null || carry > 0) {
        cl.next = new BigIntegerList(0);
        cl = cl.next;
      } else break;
    }

    return c;
  }
  public BigInteger mul(BigInteger val, int n) {
    BigInteger ret = new BigInteger(0);
    for (int i = 0; i < n; i++) {
      ret = ret.add(val);
    }
    return ret;
  }
}

class BigInt { 
  public static void main(String[] args) {
    BigInteger n = new BigInteger(1);
    for(int i = 1; i < 1000; i++) {
      n = n.mul(n, i);
      System.out.println(i + "! = " + n.asString());
    }
  }
}

// void inc_bignum(bignum *b)
// {
//   int carry = 1;
//   bignum_list *l = b->list;
//
//   for (;;) {
//     l->n += carry;
//     carry = l->n / 10;
//     l->n %= 10;
//
//     if (carry) {
//       if (l->next == NULL)
//         l->next = new_bignum_list("0");
//       l = l->next;
//     } else
//       break;
//   }
//
// }
//
// int main(void)
// {
//   bignum *b = new_bignum("999999999999999999");
//   display_bignum(b);
//   inc_bignum(b);
//   display_bignum(b);
//   free_bignum(b);
//
//   return EXIT_SUCCESS;
// }
