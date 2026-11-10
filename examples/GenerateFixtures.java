import java.io.*;
import java.nio.file.*;
import java.util.function.Consumer;
import org.skaldpdf.pdf.*;
import org.skaldpdf.geom.PageSize;
import org.skaldpdf.image.ImageData;
import org.skaldpdf.codec.*;
import org.skaldpdf.fonts.SkaldSans;
import org.skaldpdf.layout.element.*;

/** Fictional, redistributable fixtures. Run with Skald PDF 1.21.0+; no ReAI data. */
public class GenerateFixtures {
    static Path directory;
    static void write(String name, Compression compression, boolean jxl, Consumer<PdfDocument> draw) throws Exception {
        var output = new ByteArrayOutputStream();
        var writer = new PdfWriter(output, new WriterProperties(compression));
        if (jxl) writer = writer.withExperimentalJpegXl();
        try (var pdf = new PdfDocument(writer)) { draw.accept(pdf); }
        Files.write(directory.resolve(name + ".pdf"), output.toByteArray());
    }
    static void invoice(PdfDocument pdf) {
        try (var layout = new org.skaldpdf.layout.Document(pdf)) {
            layout.setFont(SkaldSans.regular()).setMargins(48);
            layout.add(new Paragraph("NORDLYS STUDIO").bold().setFontSize(28));
            layout.add(new Paragraph("FAKTURA / INVOICE 2026-1042").setFontSize(14));
            layout.add(new Paragraph("Fictional example · no customer data\nCustomer: Fjord & Fjell AS\nDate: 10 November 2026\nPayment due: 24 November 2026"));
            var table = new Table(new float[]{5,1,2}).useAllAvailableWidth();
            table.addHeaderCell("Description"); table.addHeaderCell("Qty"); table.addHeaderCell("NOK");
            for (var row : new String[][]{{"Design consultation","4","4 800,00"},{"Product photography","1","3 200,00"},{"Document automation","2","2 400,00"}})
                for (var cell : row) table.addCell(cell);
            layout.add(table);
            layout.add(new Paragraph("Subtotal: 10 400,00 NOK\nVAT / MVA 25%: 2 600,00 NOK\nTotal due: 13 000,00 NOK").bold());
            layout.add(new Paragraph("Blåbær, størrelse Ø — æøå ÆØÅ\nThank you for building things that last."));
        }
    }
    static ImageData raster(boolean alpha) {
        int w=640,h=480;var rgb=new byte[w*h*3];var mask=new byte[w*h];
        for(int y=0;y<h;y++) for(int x=0;x<w;x++) {int i=(y*w+x)*3;rgb[i]=(byte)(x*255/w);rgb[i+1]=(byte)(y*255/h);rgb[i+2]=(byte)((x+y)%256);mask[y*w+x]=(byte)(x*255/w);}
        return alpha ? ImageData.fromRgb(w,h,rgb,mask) : ImageData.fromRgb(w,h,rgb);
    }
    static void image(PdfDocument pdf,ImageData image) {var p=pdf.addNewPage(new PageSize(640,480));image.drawOn(pdf,p,0,0,640,480);}
    public static void main(String[] args) throws Exception {
        directory=Path.of(args[0]);Files.createDirectories(directory);
        write("invoice-flate",Compression.BALANCED,false,GenerateFixtures::invoice);
        write("invoice-brotli",Compression.BROTLI_MAXIMUM,false,GenerateFixtures::invoice);
        write("report-brotli",Compression.BROTLI_BALANCED,false,pdf->{try(var layout=new org.skaldpdf.layout.Document(pdf)) {
            layout.setFont(SkaldSans.regular());layout.add(new Paragraph("Quarterly activity report").bold().setFontSize(24));
            var table=new Table(new float[]{1,5,2}).useAllAvailableWidth();table.addHeaderCell("#");table.addHeaderCell("Activity");table.addHeaderCell("NOK");
            for(int i=1;i<=250;i++){table.addCell(""+i);table.addCell("Fictional project activity "+i);table.addCell("1 250,00");}layout.add(table);}});
        var raw=raster(false);
        write("raster-flate",Compression.BALANCED,false,pdf->image(pdf,raw));
        for(var q:new JpegXlOptions[]{JpegXlOptions.lossless(),JpegXlOptions.photos(),JpegXlOptions.compact()}) {
            var encoded=NativeImages.jpegXlEncode(raw,q);
            write("raster-jxl-"+(int)q.distance(),Compression.BROTLI_BALANCED,true,pdf->image(pdf,encoded));
        }
        var jpeg=RasterImages.asJpeg(raw,0.9f);
        write("jpeg-flate",Compression.BALANCED,false,pdf->image(pdf,jpeg));
        var reversible=NativeImages.jpegXlEncode(jpeg,JpegXlOptions.lossless());
        write("jpeg-reversible-jxl",Compression.BROTLI_BALANCED,true,pdf->image(pdf,reversible));
        var masked=raster(true);
        write("mask-flate",Compression.BALANCED,false,pdf->image(pdf,masked));
        var opaque=NativeImages.jpegXlEncode(raw,JpegXlOptions.lossless());
        var graySamples=new byte[640*480];for(int i=0;i<graySamples.length;i++)graySamples[i]=(byte)((i%640)*255/640);
        var gray=ImageData.fromGray(640,480,graySamples);
        write("gray-flate",Compression.BALANCED,false,pdf->image(pdf,gray));
        write("gray-jpeg",Compression.BALANCED,false,pdf->image(pdf,RasterImages.asJpeg(gray,0.9f)));
        for(var q:new JpegXlOptions[]{JpegXlOptions.lossless(),JpegXlOptions.compact()}){var encoded=NativeImages.jpegXlEncode(gray,q);write("gray-jxl-"+(int)q.distance(),Compression.BROTLI_BALANCED,true,pdf->image(pdf,encoded));}
        write("shared-image-report",Compression.BROTLI_BALANCED,true,pdf->{for(int i=0;i<16;i++)image(pdf,opaque);});
        var maskedOutput=new ByteArrayOutputStream();
        try(var pdf=new PdfDocument(new PdfReader(Files.readAllBytes(directory.resolve("mask-flate.pdf"))),new PdfWriter(maskedOutput,new WriterProperties(Compression.BROTLI_BALANCED)).withExperimentalJpegXl())){pdf.replaceImportedImage(pdf.importedImages().getFirst(),opaque);}
        Files.write(directory.resolve("mask-jxl.pdf"),maskedOutput.toByteArray());
        // External masks are represented by the ordinary PDF image wrapper.
        write("mask-brotli",Compression.BROTLI_BALANCED,false,pdf->image(pdf,masked));
        write("vectors-brotli",Compression.BROTLI_BALANCED,false,pdf->{var p=pdf.addNewPage(new PageSize(640,480));
            p.append("0.96 0.97 0.99 rg 0 0 640 480 re f\nq 60 60 520 360 re W n\n0.1 0.3 0.8 rg 40 40 260 260 re f\n0.9 0.2 0.4 rg 180 140 260 260 re f\n0.1 0.8 0.6 RG 8 w 1 J [12 6] 0 d 80 80 m 600 400 l S\nQ\nq 0.8 0.2 -0.2 0.8 420 180 cm 0.1 0.15 0.25 rg 0 0 140 140 re f Q\n");});
        write("unicode-brotli",Compression.BROTLI_BALANCED,false,pdf->{try(var layout=new org.skaldpdf.layout.Document(pdf)) {
            layout.setFont(SkaldSans.regular());layout.add(new Paragraph("Text is part of the document").bold().setFontSize(24));
            layout.add(new Paragraph("Norsk: Blåbær, æøå ÆØÅ\nΕλληνικά: Καλημέρα\nКириллица: Привет мир\nCurrency: NOK € £ $\nSearch, select and copy this text."));}});
        var legacy=Files.readAllBytes(directory.resolve("invoice-flate.pdf"));legacy[5]='1';legacy[6]='.';legacy[7]='7';Files.write(directory.resolve("rejected-legacy.pdf"),legacy);
    }
}
