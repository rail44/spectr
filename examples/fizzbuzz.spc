l: List.range(0, 100),

fizzbuzz: (i) => {
  is_fizz: i % 3 = 0,
  is_buzz: i % 5 = 0,
  fizz: if is_fizz "fizz" "",
  buzz: if is_buzz "buzz" "",
  
  fizz.concat(buzz)
},

result: l.map(fizzbuzz),

l.map((i) => [i, result[i]])
