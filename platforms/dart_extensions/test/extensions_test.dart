import 'package:canvist/canvist.dart';
import 'package:canvist_extensions/canvist_extensions.dart' as extensions;
import 'package:test/test.dart';

void main() {
  test(
    'aggregate descriptors keep their typed handles on the engine',
    () async {
      expect(identical(extensions.bold, Extensions.bold), isTrue);
      expect(identical(extensions.table, Extensions.table), isTrue);
      expect(identical(extensions.fontSize, Extensions.fontSize), isTrue);

      final builder = EngineBuilder();
      final extensions.BoldHandle bold = builder.add(extensions.bold);
      final extensions.TableHandle table = builder.add(extensions.table);
      final engine = await builder.build();
      addTearDown(engine.dispose);
      final doc = engine.parse(Format.markdown, 'One package')..selectAll();
      addTearDown(doc.dispose);

      expect(doc.run(bold.toggleBold()), isTrue);
      expect(doc.export(Format.html), '<p><strong>One package</strong></p>');
      expect(doc.query(table.tableCell()), isNull);
    },
  );
}
