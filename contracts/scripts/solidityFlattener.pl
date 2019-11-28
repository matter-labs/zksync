#!/usr/bin/perl -W
# ----------------------------------------------------------------------------------------------
# Solidity Flattener v1.0.2
#
# https://github.com/bokkypoobah/SolidityFlattener
#
# Enjoy. (c) BokkyPooBah / Bok Consulting Pty Ltd 2018. The MIT Licence.
# ----------------------------------------------------------------------------------------------

use strict;
# TODO: maybe create a json file with all contrats and gry to verify that?
# use lib qw(/Users/oleg/perl5/lib/perl5);
# use String::Escape;
use Getopt::Long qw(:config no_auto_abbrev);
use File::Basename;
use File::Spec::Functions;
use File::Path qw( make_path );


my $DEFAULTCONTRACTSDIR = "./contracts";
my $VERSION = "v1.0.3";

my $helptext = qq\
Solidity Flattener $VERSION

Usage: $0 {options}

Where options are:
  --contractsdir  Source directory for original contracts. Default '$DEFAULTCONTRACTSDIR'
  --remapdir      Remap import directories. Optional. Example "contracts/openzeppelin-solidity=node_modules/openzeppelin-solidity"
  --mainsol       Main source Solidity file. Mandatory
  --outputsol     Output flattened Solidity file. Default is mainsol with `_flattened` appended to the file name
  --verbose       Show details. Optional
  --help          Display help. Optional

Example usage:
  $0 --contractsdir=contracts --mainsol=MyContract.sol --outputsol=flattened/MyContracts_flattened.sol --verbose

Installation:
  Download solidityFlattener.pl from https://github.com/bokkypoobah/SolidityFlattener into /usr/local/bin
  chmod 755 /usr/local/bin/solidityFlattener.pl

Works on OS/X, Linux and Linux on Windows.

Enjoy. (c) BokkyPooBah / Bok Consulting Pty Ltd 2018. The MIT Licence.

Stopped\;

my ($contractsdir, $remapdir, $mainsol, $outputsol, $help, $verbose);
my %seen = ();
# my %contracts_json = ();

GetOptions(
  "contractsdir:s" => \$contractsdir,
  "remapdir:s"     => \$remapdir,
  "mainsol:s"      => \$mainsol,
  "outputsol:s"    => \$outputsol,
  "verbose"        => \$verbose,
  "help"           => \$help)
or die $helptext;

die $helptext
  if defined $help;

die $helptext
  unless defined $mainsol;

$contractsdir = $DEFAULTCONTRACTSDIR
  unless defined $contractsdir;

if (!defined $outputsol) {
  $outputsol = $mainsol;
  $outputsol =~ s/\.sol/_flattened\.sol/g;
}



if (defined $verbose) {
  printf "contractsdir: %s\n", $contractsdir;
  printf "remapdir    : %s\n", defined $remapdir ? $remapdir : "(no remapping)";
  printf "mainsol     : %s\n", $mainsol;
  printf "outputsol   : %s\n", $outputsol
}

my ( $outfile, $directories ) = fileparse $outputsol;

if ($directories ne "./") {
  make_path $directories;
}

open OUTPUT, ">$outputsol"
  or die "Cannot open $outputsol for writing. Stopped";

processSol(catfile($contractsdir, $mainsol), 0);

# my $outputjson = "${outputsol}_json";
# open JSON_OUTPUT, ">$outputjson"
#   or die "Cannot open $outputjson for writing. Stopped";

# print JSON_OUTPUT "{\n";
# foreach my $key (keys %contracts_json) {
#   # do whatever you want with $key and $value here ...
#   my $value = $contracts_json{$key};
#   printf JSON_OUTPUT "\t\"%s\": \"%s\",\n", $key, $value;
# }
# print JSON_OUTPUT "}";

close OUTPUT
  or die "Cannot close $outputsol. Stopped";

exit;


# ------------------------------------------------------------------------------
# Process Solidity file
# ------------------------------------------------------------------------------
sub processSol {
  my ($sol, $level) = @_;
  if (defined $remapdir) {
    my ($splitfrom, $splitto) = split /=/, $remapdir;
    # printf "%sSplit %s: %s => %s\n", "    " x $level, $remapdir, $splitfrom, $splitto
    #   if defined $verbose;
    if ($sol =~ /$splitfrom/) {
      printf "%sRemapping %s\n", "    " x $level, $sol
        if defined $verbose;
      $sol =~ s!$splitfrom!$splitto!;
      printf "%s       to %s\n", "    " x $level, $sol
        if defined $verbose;
    }
  }
  my $dir = dirname($sol);
  my $file = basename($sol);
  $seen{$file} = 1;
  printf "%sProcessing %s\n", "    " x $level, $sol
    if defined $verbose;

  open INPUT, "<$sol"
    or die "Cannot open $sol for reading. Stopped";
  my @lines = <INPUT>;
  close INPUT
    or die "Cannot close $sol. Stopped";

  # my $concat_file = join("", @lines);
  # $concat_file = String::Escape::backslash( $concat_file );
  # $contracts_json{$file} = $concat_file;

  for my $line (@lines) {
    chomp $line;
    if ($line =~ /^import/) {
      my $importfile = $line;
      $importfile =~ s/import [\"\']//;
      $importfile =~ s/[\"\'];.*$//;
      $file = basename($importfile);
      if ($seen{$file}) {
        printf "%s    Already Imported %s\n", "    " x $level, catfile($dir, $importfile)
          if defined $verbose;
      } else {
        printf "%s    Importing %s\n", "    " x $level, catfile($dir, $importfile)
          if defined $verbose;
        processSol(catfile($dir, $importfile), $level + 1)
      }
    } else {
      if ($level == 0 || !($line =~ /^pragma/)) {
        printf OUTPUT "%s\n", $line;
      }
    }
  }
}
